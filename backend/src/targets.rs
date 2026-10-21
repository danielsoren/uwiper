//! Things UWiper can remove: files and folders, registry keys and values,
//! services and scheduled tasks. Each one is checked against the safety rules,
//! can be backed up into a quarantine folder first and put back from there.
#![cfg_attr(not(windows), allow(dead_code))]

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::util::{canonical, expand_env, is_within, norm_path, windows_dir};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Target {
    /// A file or a folder with everything in it.
    Path { path: String },
    /// A registry key with everything below it, e.g. `HKCU\Software\Vendor\App`.
    /// 32-bit keys use their real path under `WOW6432Node`.
    RegistryKey { key: String },
    /// One value of a registry key, such as a Run entry.
    RegistryValue { key: String, name: String },
    /// A Windows service, by its key name.
    Service { name: String },
    /// A scheduled task, by its full path such as `\Vendor\Updater`.
    Task { path: String },
    /// One folder in the user's ("user") or everyone's ("machine") PATH.
    PathEntry { scope: String, entry: String },
}

/// What was kept so a removed target can be put back.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Backup {
    /// The file or folder itself, moved into the batch.
    Moved { file: String },
    /// A `.reg` export (registry keys and services).
    Reg { file: String },
    /// A registry value's type and raw data.
    Value { vtype: u32, data: Vec<u8> },
    /// The task's XML definition.
    TaskXml { file: String },
    /// Nothing to keep: the target itself says what to put back.
    Entry,
}

/// What the safety rules need to know.
pub struct Guard {
    /// Folders that are never removed themselves, nor anything that holds them.
    pub protected: Vec<String>,
    /// UWiper's own folders: nothing in them is touched either.
    pub own: Vec<String>,
    /// Start menu folders, whose own path contains "Microsoft\Windows".
    pub start_menus: Vec<String>,
    /// Windows and Microsoft components stay off limits.
    pub protect_system: bool,
    /// The user's "Never touch" list: folders (canonical) and registry keys (`norm_key`).
    pub exclusions: Vec<String>,
}

/// A registry key in one spelling: `HKCU`/`HKLM`, no "Computer\" prefix,
/// no WOW6432Node, lowercase. Only for comparing keys.
pub fn norm_key(key: &str) -> String {
    let k = norm_path(key);
    let k = k.strip_prefix(r"computer\").unwrap_or(&k);
    let k = k
        .replacen("hkey_current_user", "hkcu", 1)
        .replacen("hkey_local_machine", "hklm", 1);
    k.replacen(r"\software\wow6432node", r"\software", 1)
}

fn looks_like_key(entry: &str) -> bool {
    let e = entry.trim().to_ascii_uppercase();
    let e = e.strip_prefix("COMPUTER\\").unwrap_or(&e);
    e.starts_with("HK")
}

impl Guard {
    pub fn new(settings: &crate::settings::Settings) -> Self {
        let mut own = vec![crate::paths::home().to_string_lossy().into_owned()];
        for key in crate::paths::DATA_DIRS {
            own.push(crate::paths::data_dir(settings, key).to_string_lossy().into_owned());
        }
        // Long forms, since %TEMP% and friends often hold 8.3 names.
        let clean = |list: Vec<String>| list.iter().map(|p| canonical(p)).filter(|p| !p.is_empty()).collect();
        Guard {
            protected: clean(protected_folders()),
            own: clean(own),
            start_menus: clean(start_menus()),
            protect_system: settings.safety.protect_system,
            exclusions: settings
                .scan
                .exclusions
                .iter()
                .map(|e| {
                    if looks_like_key(e) {
                        norm_key(e)
                    } else {
                        canonical(&expand_env(e))
                    }
                })
                .filter(|e| !e.is_empty())
                .collect(),
        }
    }

    /// True when a path or key is on the "Never touch" list, or holds something that is.
    pub fn is_excluded(&self, path_or_key: &str) -> bool {
        let item = if looks_like_key(path_or_key) {
            norm_key(path_or_key)
        } else {
            canonical(path_or_key)
        };
        // Removing a parent of an excluded item would remove the item too.
        self.exclusions
            .iter()
            .any(|e| is_within(&item, e) || is_within(e, &item))
    }

    /// `Err` with the reason when the target must not be touched.
    pub fn check(&self, target: &Target) -> std::result::Result<(), String> {
        match target {
            Target::Path { path } => self.check_path(path),
            Target::RegistryKey { key } => self.check_key(key),
            Target::RegistryValue { key, name } => {
                split_key(key).ok_or("Not a registry key UWiper works with")?;
                if name.is_empty() {
                    return Err("The default value is left alone".into());
                }
                self.check_excluded(&format!(r"{key}\{name}"))
            }
            Target::Service { name } => {
                if name.is_empty() || name.contains('\\') {
                    return Err("Not a service name".into());
                }
                Ok(())
            }
            Target::Task { path } => {
                if !path.starts_with('\\') || path.len() < 2 {
                    return Err("Not a scheduled task path".into());
                }
                if self.protect_system && norm_path(path).starts_with(r"\microsoft\") {
                    return Err("Windows task".into());
                }
                Ok(())
            }
            Target::PathEntry { scope, entry } => {
                if !matches!(scope.as_str(), "user" | "machine") || entry.trim().is_empty() || entry.contains(';') {
                    return Err("Not a PATH entry".into());
                }
                Ok(())
            }
        }
    }

    fn check_excluded(&self, path_or_key: &str) -> std::result::Result<(), String> {
        if self.is_excluded(path_or_key) {
            return Err("On your “Never touch” list".into());
        }
        Ok(())
    }

    fn check_path(&self, path: &str) -> std::result::Result<(), String> {
        if path.trim() != path {
            return Err("An unusual path spelling".into());
        }
        let raw = path.replace('/', "\\");
        let below_root = raw.len() > 3 && raw.as_bytes()[1] == b':' && raw.as_bytes()[2] == b'\\';
        if !below_root {
            return Err("Drive roots and relative paths are never removed".into());
        }
        // Windows quietly drops "." and "..", doubled separators and trailing
        // dots or spaces, so such a spelling could point somewhere else.
        let odd = raw[3..]
            .trim_end_matches('\\')
            .split('\\')
            .any(|part| part.is_empty() || part == "." || part == ".." || part.ends_with('.') || part.ends_with(' '));
        if odd {
            return Err("An unusual path spelling".into());
        }
        let p = canonical(&raw);
        if is_within(&p, &canonical(&windows_dir())) {
            return Err("Part of Windows".into());
        }
        if self.protected.iter().any(|root| is_within(root, &p)) {
            return Err("A system or shared folder".into());
        }
        if self.own.iter().any(|dir| is_within(dir, &p) || is_within(&p, dir)) {
            return Err("UWiper's own data".into());
        }
        // Inside a start menu only the part below it counts.
        let from = self
            .start_menus
            .iter()
            .find(|m| is_within(&p, m))
            .map_or(0, |m| m.len());
        let microsoft = p[from..]
            .split('\\')
            .skip(1)
            .any(|part| part.starts_with("microsoft") || part.starts_with("windows"));
        if self.protect_system && microsoft {
            return Err("Windows or Microsoft component".into());
        }
        self.check_excluded(&p)
    }

    fn check_key(&self, key: &str) -> std::result::Result<(), String> {
        let (_, sub) = split_key(key).ok_or("Not a registry key UWiper works with")?;
        let parts: Vec<String> = norm_path(sub).split('\\').map(str::to_string).collect();
        let first = parts.first().map(String::as_str).unwrap_or_default();
        if first != "software" || parts.len() < 2 {
            return Err("Only keys under Software are removed".into());
        }
        // Skip WOW6432Node so 32-bit keys follow the same rules.
        let rest: Vec<&str> = parts[1..]
            .iter()
            .map(String::as_str)
            .skip_while(|p| *p == "wow6432node")
            .collect();
        let Some(&vendor) = rest.first() else {
            return Err("A shared registry key".into());
        };
        if [
            "classes",
            "policies",
            "clients",
            "registeredapplications",
            "wow6432node",
        ]
        .contains(&vendor)
        {
            return Err("A shared registry key".into());
        }
        let uninstall = r"microsoft\windows\currentversion\uninstall";
        let joined = rest.join("\\");
        let app_paths = r"microsoft\windows\currentversion\app paths";
        // An app's own entry in these Windows lists is fine to remove.
        let is_app_entry = rest.len() == 5
            && (joined.starts_with(&format!("{uninstall}\\")) || joined.starts_with(&format!("{app_paths}\\")));
        // Microsoft's keys are never removed, whatever the settings say.
        let system_vendor = vendor == "microsoft" || vendor == "windows";
        if system_vendor && !is_app_entry {
            return Err("Windows or Microsoft component".into());
        }
        self.check_excluded(key)
    }
}

/// Folders that hold many things: removing one of them, or anything that
/// contains one, is never right.
fn protected_folders() -> Vec<String> {
    let vars = [
        "SystemRoot",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramW6432",
        "CommonProgramFiles",
        "CommonProgramFiles(x86)",
        "CommonProgramW6432",
        "ProgramData",
        "ALLUSERSPROFILE",
        "PUBLIC",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "TEMP",
        "OneDrive",
    ];
    let mut out: Vec<String> = vars.iter().filter_map(|v| std::env::var(v).ok()).collect();
    if let Ok(drive) = std::env::var("SystemDrive") {
        out.push(format!(r"{drive}\Users"));
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        for dir in [
            "Desktop",
            "Documents",
            "Downloads",
            "Pictures",
            "Music",
            "Videos",
            "Saved Games",
            "AppData\\LocalLow",
        ] {
            out.push(format!(r"{profile}\{dir}"));
        }
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        out.push(format!(r"{local}\Programs"));
    }
    out.extend(start_menus());
    out.push(expand_env(r"%PUBLIC%\Desktop"));
    out
}

fn start_menus() -> Vec<String> {
    ["%APPDATA%", "%ProgramData%"]
        .iter()
        .map(|root| expand_env(&format!(r"{root}\Microsoft\Windows\Start Menu\Programs")))
        .filter(|p| !p.contains('%'))
        .collect()
}

/// `HKCU\Software\X` → (`HKCU`, `Software\X`)
pub fn split_key(key: &str) -> Option<(&'static str, &str)> {
    let (hive, sub) = key.split_once('\\')?;
    let hive = match hive.to_ascii_uppercase().as_str() {
        "HKCU" | "HKEY_CURRENT_USER" => "HKCU",
        "HKLM" | "HKEY_LOCAL_MACHINE" => "HKLM",
        _ => return None,
    };
    (!sub.trim_matches('\\').is_empty()).then_some((hive, sub.trim_matches('\\')))
}

/// Folders and machine-wide registry keys need administrator rights.
pub fn needs_admin(target: &Target) -> bool {
    match target {
        Target::Path { path } => {
            let profile = std::env::var("USERPROFILE").unwrap_or_default();
            profile.is_empty() || !is_within(path, &profile)
        }
        Target::RegistryKey { key } | Target::RegistryValue { key, .. } => {
            split_key(key).is_none_or(|(hive, _)| hive == "HKLM")
        }
        Target::Service { .. } | Target::Task { .. } => true,
        Target::PathEntry { scope, .. } => scope == "machine",
    }
}

/// A short, human label for a target.
pub fn describe(target: &Target) -> String {
    match target {
        Target::Path { path } => path.clone(),
        Target::RegistryKey { key } => key.clone(),
        Target::RegistryValue { key, name } => format!(r"{key}\{name}"),
        Target::Service { name } => format!("Service {name}"),
        Target::Task { path } => format!("Task {path}"),
        Target::PathEntry { scope, entry } => {
            format!("{} PATH: {entry}", if scope == "machine" { "System" } else { "User" })
        }
    }
}

/// The PATH text without `entry`, or `None` when it isn't there.
pub fn without_entry(path: &str, entry: &str) -> Option<String> {
    let parts: Vec<&str> = path.split(';').collect();
    let kept: Vec<&str> = parts.iter().copied().filter(|p| !same_entry(p, entry)).collect();
    (kept.len() < parts.len()).then(|| kept.join(";"))
}

/// The PATH text with `entry` added at the end, or `None` when it's there already.
pub fn with_entry(path: &str, entry: &str) -> Option<String> {
    if path.split(';').any(|p| same_entry(p, entry)) {
        return None;
    }
    let base = path.trim_end_matches(';');
    Some(if base.is_empty() {
        entry.to_string()
    } else {
        format!("{base};{entry}")
    })
}

fn same_entry(a: &str, b: &str) -> bool {
    let clean = |s: &str| s.trim().trim_end_matches('\\').to_lowercase();
    !a.trim().is_empty() && clean(a) == clean(b)
}

// ---------------------------------------------------------------------------
// Files and folders. These work on every OS so they can be tested anywhere.

/// Moves a file or folder, copying when it has to cross drives. A copy that
/// fails half-way is undone; the original is removed only after a full copy.
/// `Ok(false)` means everything was copied but some originals were in use
/// and stayed behind.
pub fn move_path(from: &Path, to: &Path) -> Result<bool> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Moving to another drive: ERROR_NOT_SAME_DEVICE on Windows, EXDEV elsewhere.
    const CROSS_DEVICE: i32 = if cfg!(windows) { 17 } else { 18 };
    match std::fs::rename(from, to) {
        Ok(()) => return Ok(true),
        Err(e) if e.raw_os_error() == Some(CROSS_DEVICE) => {}
        Err(e) => return Err(friendly_io(e)),
    }
    if let Err(e) = copy_all(from, to) {
        let _ = remove_path(to);
        return Err(friendly_io(e));
    }
    Ok(remove_path(from).is_ok())
}

/// Moves what's in `from` into the existing folder `to`. Files already there
/// win; returns how many were in the way, and their copies stay in `from`.
fn merge_into(from: &Path, to: &Path) -> Result<usize> {
    let mut conflicts = 0;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        match std::fs::symlink_metadata(&dest) {
            Err(_) => {
                move_path(&entry.path(), &dest)?;
            }
            Ok(meta) if meta.is_dir() && entry.path().is_dir() => conflicts += merge_into(&entry.path(), &dest)?,
            Ok(_) => conflicts += 1,
        }
    }
    // Only an emptied folder goes away.
    let _ = std::fs::remove_dir(from);
    Ok(conflicts)
}

fn copy_all(from: &Path, to: &Path) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(from)?;
    if meta.file_type().is_symlink() {
        return Err(std::io::Error::other(
            "It holds a link or junction, so it can only be moved within its own drive",
        ));
    }
    if meta.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy_all(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(from, to).map(|_| ())
    }
}

pub fn remove_path(path: &Path) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        // Read-only files can't be deleted until the flag is cleared.
        if meta.permissions().readonly() {
            let mut perms = meta.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            let _ = std::fs::set_permissions(path, perms);
        }
        std::fs::remove_file(path)
    }
}

fn friendly_io(e: std::io::Error) -> Error {
    let in_use = || Error::Other("In use – close the app that uses it and try again".into());
    match e.raw_os_error() {
        // With admin rights, "access denied" means something holds the file open.
        Some(5) if crate::system::is_elevated() => in_use(),
        Some(5) => Error::NeedsAdmin,
        Some(32) | Some(33) => in_use(),
        _ => Error::Io(e),
    }
}

// ---------------------------------------------------------------------------
// Removing, backing up and restoring.

/// Removes the target and keeps a backup of it in `dir`. The note says when
/// it was only partly removed.
pub fn quarantine(target: &Target, dir: &Path) -> Result<(Backup, Option<String>)> {
    std::fs::create_dir_all(dir)?;
    match target {
        Target::Path { path } => {
            let source = PathBuf::from(path);
            if std::fs::symlink_metadata(&source).is_err() {
                return Err(Error::Other("Already gone".into()));
            }
            let name = source
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "item".into());
            let complete = move_path(&source, &dir.join(&name))?;
            let note = (!complete).then(|| "Some files were in use and stayed behind".to_string());
            Ok((Backup::Moved { file: name }, note))
        }
        _ => imp::quarantine(target, dir),
    }
}

/// Removes the target for good.
pub fn delete(target: &Target) -> Result<()> {
    match target {
        Target::Path { path } => remove_path(Path::new(path)).map_err(friendly_io),
        _ => imp::delete(target),
    }
}

/// Puts a quarantined target back.
pub fn restore(target: &Target, backup: &Backup, dir: &Path) -> Result<()> {
    match (target, backup) {
        (Target::Path { path }, Backup::Moved { file }) => {
            let (stored, original) = (dir.join(file), PathBuf::from(path));
            if std::fs::symlink_metadata(&stored).is_err() {
                return Err(Error::Other("The quarantined copy is missing".into()));
            }
            match std::fs::symlink_metadata(&original) {
                Err(_) => move_path(&stored, &original).map(|_| ()),
                // Partly removed, or the app was installed again: fill in what's missing.
                Ok(meta) if meta.is_dir() && stored.is_dir() => match merge_into(&stored, &original)? {
                    0 => Ok(()),
                    n => Err(Error::Other(format!(
                        "{n} files already exist at the original location – the rest is back, \
                         and the quarantined copies of those are kept"
                    ))),
                },
                Ok(_) => Err(Error::Other("Something new is already at the original location".into())),
            }
        }
        _ => imp::restore(target, backup, dir),
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn quarantine(_target: &Target, _dir: &Path) -> Result<(Backup, Option<String>)> {
        Err(Error::Unsupported)
    }

    pub fn delete(_target: &Target) -> Result<()> {
        Err(Error::Unsupported)
    }

    pub fn restore(_target: &Target, _backup: &Backup, _dir: &Path) -> Result<()> {
        Err(Error::Unsupported)
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::util::run_tool;
    use winreg::enums::*;
    use winreg::{RegKey, RegValue};

    fn hive(name: &str) -> RegKey {
        RegKey::predef(if name == "HKLM" {
            HKEY_LOCAL_MACHINE
        } else {
            HKEY_CURRENT_USER
        })
    }

    fn registry_error(e: std::io::Error) -> Error {
        match e.kind() {
            std::io::ErrorKind::NotFound => Error::Other("Already gone".into()),
            std::io::ErrorKind::PermissionDenied => Error::NeedsAdmin,
            _ => Error::Io(e),
        }
    }

    fn export(key: &str, file: &Path) -> Result<()> {
        run_tool("reg.exe", &["export", key, &file.to_string_lossy(), "/y"]).map(|_| ())
    }

    fn import(file: &Path) -> Result<()> {
        run_tool("reg.exe", &["import", &file.to_string_lossy()]).map(|_| ())
    }

    fn service_key(name: &str) -> String {
        format!(r"HKLM\SYSTEM\CurrentControlSet\Services\{name}")
    }

    fn task_file(path: &str) -> PathBuf {
        PathBuf::from(format!(r"{}\System32\Tasks{}", windows_dir(), path))
    }

    pub fn quarantine(target: &Target, dir: &Path) -> Result<(Backup, Option<String>)> {
        match target {
            Target::RegistryKey { key } => {
                let file = "key.reg";
                export(key, &dir.join(file))?;
                // Deleting a tree can stop half-way at a protected subkey, so
                // once exported, the backup is kept whatever happens.
                let note = delete(target)
                    .err()
                    .map(|e| format!("Only partly removed ({e}) – its backup is kept"));
                Ok((Backup::Reg { file: file.into() }, note))
            }
            Target::RegistryValue { key, name } => {
                let (h, sub) = split_key(key).ok_or(Error::Other("Not a registry key".into()))?;
                let reg = hive(h)
                    .open_subkey_with_flags(sub, KEY_READ | KEY_SET_VALUE)
                    .map_err(registry_error)?;
                let value = reg.get_raw_value(name).map_err(registry_error)?;
                reg.delete_value(name).map_err(registry_error)?;
                Ok((
                    Backup::Value {
                        vtype: value.vtype as u32,
                        data: value.bytes.into_owned(),
                    },
                    None,
                ))
            }
            Target::Service { name } => {
                let file = "service.reg";
                export(&service_key(name), &dir.join(file))?;
                delete(target)?;
                Ok((Backup::Reg { file: file.into() }, None))
            }
            Target::Task { path } => {
                let file = "task.xml";
                std::fs::copy(task_file(path), dir.join(file)).map_err(friendly_io)?;
                delete(target)?;
                Ok((Backup::TaskXml { file: file.into() }, None))
            }
            Target::PathEntry { .. } => {
                delete(target)?;
                Ok((Backup::Entry, None))
            }
            Target::Path { .. } => unreachable!("paths are handled by the caller"),
        }
    }

    const MACHINE_ENV: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";

    /// Reads PATH with its value type (it's usually REG_EXPAND_SZ).
    fn read_path(scope: &str) -> Result<(RegKey, String, RegType)> {
        let key = if scope == "machine" {
            RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(MACHINE_ENV, KEY_READ | KEY_SET_VALUE)
        } else {
            RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags("Environment", KEY_READ | KEY_SET_VALUE)
        }
        .map_err(registry_error)?;
        let raw = key.get_raw_value("Path").map_err(registry_error)?;
        let units: Vec<u16> = raw
            .bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let text = String::from_utf16_lossy(&units).trim_end_matches('\0').to_string();
        Ok((key, text, raw.vtype))
    }

    fn write_path(key: &RegKey, text: &str, vtype: RegType) -> Result<()> {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
        };
        let bytes: Vec<u8> = text.encode_utf16().chain(Some(0)).flat_map(u16::to_le_bytes).collect();
        key.set_raw_value(
            "Path",
            &RegValue {
                bytes: bytes.into(),
                vtype,
            },
        )
        .map_err(registry_error)?;
        // Tell running programs (Explorer, new terminals) that PATH changed.
        let what: Vec<u16> = "Environment".encode_utf16().chain(Some(0)).collect();
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                0,
                what.as_ptr() as isize,
                SMTO_ABORTIFHUNG,
                3000,
                std::ptr::null_mut(),
            );
        }
        Ok(())
    }

    pub fn delete(target: &Target) -> Result<()> {
        match target {
            Target::RegistryKey { key } => {
                let (h, sub) = split_key(key).ok_or(Error::Other("Not a registry key".into()))?;
                hive(h).delete_subkey_all(sub).map_err(registry_error)
            }
            Target::RegistryValue { key, name } => {
                let (h, sub) = split_key(key).ok_or(Error::Other("Not a registry key".into()))?;
                let reg = hive(h)
                    .open_subkey_with_flags(sub, KEY_SET_VALUE)
                    .map_err(registry_error)?;
                reg.delete_value(name).map_err(registry_error)
            }
            Target::Service { name } => {
                // Deleting a running service only marks it, and restoring it
                // before a restart would keep that mark. Stop it first.
                if run_tool("sc.exe", &["stop", name]).is_ok() {
                    for _ in 0..20 {
                        let state = run_tool("sc.exe", &["query", name]).unwrap_or_default();
                        if state.is_empty() || state.contains("STOPPED") {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    }
                }
                run_tool("sc.exe", &["delete", name]).map(|_| ())
            }
            Target::Task { path } => run_tool("schtasks.exe", &["/Delete", "/TN", path, "/F"]).map(|_| ()),
            Target::PathEntry { scope, entry } => {
                let (key, text, vtype) = read_path(scope)?;
                let next = without_entry(&text, entry).ok_or(Error::Other("Already gone".into()))?;
                write_path(&key, &next, vtype)
            }
            Target::Path { .. } => unreachable!("paths are handled by the caller"),
        }
    }

    fn reg_type(vtype: u32) -> RegType {
        match vtype {
            1 => REG_SZ,
            2 => REG_EXPAND_SZ,
            3 => REG_BINARY,
            4 => REG_DWORD,
            5 => REG_DWORD_BIG_ENDIAN,
            6 => REG_LINK,
            7 => REG_MULTI_SZ,
            8 => REG_RESOURCE_LIST,
            9 => REG_FULL_RESOURCE_DESCRIPTOR,
            10 => REG_RESOURCE_REQUIREMENTS_LIST,
            11 => REG_QWORD,
            _ => REG_NONE,
        }
    }

    pub fn restore(target: &Target, backup: &Backup, dir: &Path) -> Result<()> {
        match (target, backup) {
            (Target::RegistryKey { .. } | Target::Service { .. }, Backup::Reg { file }) => import(&dir.join(file)),
            (Target::RegistryValue { key, name }, Backup::Value { vtype, data }) => {
                let (h, sub) = split_key(key).ok_or(Error::Other("Not a registry key".into()))?;
                let (reg, _) = hive(h).create_subkey(sub).map_err(registry_error)?;
                let value = RegValue {
                    bytes: data.as_slice().into(),
                    vtype: reg_type(*vtype),
                };
                reg.set_raw_value(name, &value).map_err(registry_error)
            }
            (Target::Task { path }, Backup::TaskXml { file }) => {
                let xml = dir.join(file);
                run_tool(
                    "schtasks.exe",
                    &["/Create", "/TN", path, "/XML", &xml.to_string_lossy(), "/F"],
                )
                .map(|_| ())
            }
            (Target::PathEntry { scope, entry }, Backup::Entry) => {
                let (key, text, vtype) = read_path(scope)?;
                match with_entry(&text, entry) {
                    Some(next) => write_path(&key, &next, vtype),
                    None => Ok(()),
                }
            }
            _ => Err(Error::Other("This backup doesn't match the item".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard() -> Guard {
        Guard {
            protected: [r"C:\Program Files", r"C:\Users\me", r"C:\Users\me\AppData\Roaming"]
                .iter()
                .map(|p| norm_path(p))
                .collect(),
            own: vec![norm_path(r"D:\Tools\UWiper")],
            start_menus: vec![norm_path(
                r"C:\Users\me\AppData\Roaming\Microsoft\Windows\Start Menu\Programs",
            )],
            protect_system: true,
            exclusions: vec![norm_path(r"D:\Games"), norm_key(r"HKCU\Software\Keep")],
        }
    }

    fn path(p: &str) -> Target {
        Target::Path { path: p.into() }
    }

    fn key(k: &str) -> Target {
        Target::RegistryKey { key: k.into() }
    }

    #[test]
    fn allows_app_folders_and_blocks_shared_ones() {
        let g = guard();
        assert!(g.check(&path(r"C:\Program Files\Vendor\App")).is_ok());
        assert!(g.check(&path(r"C:\Users\me\AppData\Roaming\App")).is_ok());
        assert!(g.check(&path(r"C:\Program Files")).is_err());
        assert!(g.check(&path(r"C:\Program Files\")).is_err());
        assert!(g.check(&path(r"C:\Users")).is_err(), "holds a protected folder");
        assert!(g.check(&path(r"C:\")).is_err());
        assert!(g.check(&path(r"relative\path")).is_err());
        assert!(g.check(&path(r"C:\Users\me\AppData\Roaming\Microsoft\Teams")).is_err());
        assert!(
            g.check(&path(r"D:\Tools\UWiper\quarantine")).is_err(),
            "UWiper's own data"
        );
        let menu = r"C:\Users\me\AppData\Roaming\Microsoft\Windows\Start Menu\Programs";
        assert!(g.check(&path(&format!(r"{menu}\App.lnk"))).is_ok());
        assert!(g.check(&path(&format!(r"{menu}\Microsoft Office Tools"))).is_err());
    }

    #[test]
    fn rejects_paths_windows_would_rewrite() {
        let g = guard();
        for p in [
            r"C:\Program Files.",
            r"C:\Program Files\.",
            r"C:\Program Files\x\..",
            r"C:\\Program Files",
            r"C:\Program Files\App ",
            r"C:\Users\me\Documents.",
        ] {
            assert!(g.check(&path(p)).is_err(), "{p}");
        }
        assert!(
            g.check(&path(r"C:\Program Files\App\")).is_ok(),
            "a trailing separator is fine"
        );
    }

    #[test]
    fn respects_the_never_touch_list() {
        let g = guard();
        assert!(g.check(&path(r"D:\Games\Old")).is_err());
        assert!(g.check(&path(r"D:\Games")).is_err());
        assert!(g.check(&path(r"D:\")).is_err());
        assert!(g.check(&key(r"HKCU\Software\Keep\Sub")).is_err());
        assert!(g.is_excluded(r"Computer\HKEY_CURRENT_USER\Software\Keep"));
        let g = Guard {
            exclusions: vec![norm_key(r"HKLM\SOFTWARE\Vendor")],
            ..guard()
        };
        assert!(
            g.check(&key(r"HKLM\SOFTWARE\WOW6432Node\Vendor\App")).is_err(),
            "the 32-bit twin too"
        );
    }

    #[test]
    fn edits_path_entries() {
        let path = r"C:\Windows;C:\Gone\bin\;%USERPROFILE%\tools";
        assert_eq!(
            without_entry(path, r"c:\gone\BIN").as_deref(),
            Some(r"C:\Windows;%USERPROFILE%\tools")
        );
        assert_eq!(without_entry(path, r"C:\Other"), None);
        assert_eq!(
            with_entry(r"C:\Windows;", r"C:\Gone\bin").as_deref(),
            Some(r"C:\Windows;C:\Gone\bin")
        );
        assert_eq!(with_entry(path, r"C:\Gone\bin"), None);
        assert!(guard()
            .check(&Target::PathEntry {
                scope: "user".into(),
                entry: "a;b".into()
            })
            .is_err());
    }

    #[test]
    fn allows_vendor_keys_only() {
        let g = guard();
        assert!(g.check(&key(r"HKCU\Software\Vendor\App")).is_ok());
        assert!(g.check(&key(r"HKLM\SOFTWARE\WOW6432Node\Vendor")).is_ok());
        assert!(g.check(&key(r"HKCU\Software")).is_err());
        assert!(g.check(&key(r"HKLM\SOFTWARE\WOW6432Node")).is_err());
        assert!(g.check(&key(r"HKCU\Software\Classes\.txt")).is_err());
        assert!(g.check(&key(r"HKLM\SOFTWARE\Microsoft\Office")).is_err());
        assert!(g.check(&key(r"HKLM\SYSTEM\CurrentControlSet\Services\x")).is_err());
        assert!(g.check(&key(r"HKCR\x\y")).is_err());
        assert!(g
            .check(&key(
                r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\{GUID}"
            ))
            .is_ok());
        assert!(g
            .check(&key(r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"))
            .is_err());
        let app_path = r"HKCU\Software\Microsoft\Windows\CurrentVersion\App Paths\gone.exe";
        assert!(g.check(&key(app_path)).is_ok());
        assert!(g
            .check(&key(r"HKCU\Software\Microsoft\Windows\CurrentVersion\App Paths"))
            .is_err());
    }

    #[test]
    fn moves_folders_and_puts_them_back() {
        let root = std::env::temp_dir().join(format!("uwiper-move-{}", std::process::id()));
        let original = root.join("App");
        std::fs::create_dir_all(original.join("sub")).unwrap();
        std::fs::write(original.join("sub/a.txt"), b"hello").unwrap();
        let batch = root.join("q/1");

        let target = path(&original.to_string_lossy());
        let (backup, note) = quarantine(&target, &batch).unwrap();
        assert!(note.is_none());
        assert!(!original.exists());
        assert_eq!(std::fs::read(batch.join("App/sub/a.txt")).unwrap(), b"hello");

        // The app came back meanwhile: restoring keeps its files and adds the rest.
        std::fs::create_dir_all(&original).unwrap();
        std::fs::write(original.join("new.txt"), b"new").unwrap();
        restore(&target, &backup, &batch).unwrap();
        assert_eq!(std::fs::read(original.join("sub/a.txt")).unwrap(), b"hello");
        assert_eq!(std::fs::read(original.join("new.txt")).unwrap(), b"new");
        assert!(restore(&target, &backup, &batch).is_err(), "nothing left to restore");

        // A file that came back meanwhile wins, and the quarantined copy is kept.
        let (backup, _) = quarantine(&target, &batch).unwrap();
        std::fs::create_dir_all(original.join("sub")).unwrap();
        std::fs::write(original.join("sub/a.txt"), b"newer").unwrap();
        assert!(restore(&target, &backup, &batch).is_err());
        assert_eq!(std::fs::read(original.join("sub/a.txt")).unwrap(), b"newer");
        assert_eq!(std::fs::read(original.join("new.txt")).unwrap(), b"new");
        assert_eq!(std::fs::read(batch.join("App/sub/a.txt")).unwrap(), b"hello");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
