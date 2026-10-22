//! Entries without an owner: things left behind by apps that are already
//! gone. Data folders and settings no installed app claims, apps still
//! listed as installed whose files are gone, shortcuts and PATH entries that
//! point nowhere, and startup entries, services and tasks whose programs
//! are missing.
//!
//! Finding what nothing owns is guesswork, so the guesses are labelled:
//! a missing file is certain (high), an empty folder likely (medium), a
//! folder or key no app claims only possible (low) – those are never
//! picked for you, and each shows when it last changed.
#![cfg_attr(not(windows), allow(dead_code))]

use std::path::Path;

use serde::Serialize;
use tauri::State;

use crate::apps::{self, InstalledApp};
use crate::error::{blocking, Error, Result};
use crate::leftovers::{
    clean_name, clean_publisher, file_name, shortcut_dirs, squash, subdirs, Found, Leftover, GENERIC,
};
use crate::settings::Settings;
use crate::targets::{Guard, Target};
use crate::util::{canonical, expand_env, split_command};
use crate::AppState;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OrphanScan {
    items: Vec<Leftover>,
    /// Scheduled tasks could only be read with administrator rights.
    needs_admin: bool,
}

/// Every entry of the user's and the system PATH, as written.
pub fn path_entries() -> Vec<(&'static str, String)> {
    imp::path_entries()
}

#[tauri::command]
pub async fn scan_orphans(state: State<'_, AppState>) -> Result<OrphanScan> {
    let settings = state.settings.lock().unwrap().clone();
    blocking(move || scan(&settings)).await
}

// ---------------------------------------------------------------------------
// Who owns what

/// Names and folders that belong to something installed or running.
struct Owners {
    /// Squashed app names, publishers, folder names and exe names.
    tokens: Vec<String>,
    /// Install folders, and folders of programs that run (services, tasks,
    /// startup entries): nothing that holds one is ever an orphan.
    folders: Vec<String>,
}

/// Folders under these hold one app each; the parts above them say nothing.
const ROOT_NAMES: [&str; 8] = [
    "program files",
    "program files (x86)",
    "programdata",
    "local",
    "roaming",
    "locallow",
    "programs",
    "common files",
];

impl Owners {
    fn new(apps: &[InstalledApp], programs: &[String]) -> Self {
        let mut tokens = Vec::new();
        let mut folders = Vec::new();
        let shared = apps::shared_folders();
        let mut add = |t: String| {
            if t.len() >= 3 && !GENERIC.contains(&t.as_str()) && !tokens.contains(&t) {
                tokens.push(t);
            }
        };
        for app in apps {
            let clean = clean_name(&app.name);
            add(squash(&clean));
            let publisher = app.publisher.as_deref().map(clean_publisher).unwrap_or_default();
            add(publisher.clone());
            if let Some((first, rest)) = clean.split_once(' ') {
                if squash(first) == publisher {
                    add(squash(rest));
                }
            }
            let location = app
                .install_location
                .clone()
                .or_else(|| apps::location_from_uninstaller(app.uninstall_string.as_deref(), &shared));
            if let Some(dir) = location {
                add(squash(&clean_name(&file_name(Path::new(dir.trim_end_matches('\\'))))));
                // Data folders are often named after the exe (VS Code keeps %APPDATA%\Code).
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for e in entries.flatten() {
                        let p = e.path();
                        if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("exe"))
                            && !crate::icons::is_helper_exe(&p)
                        {
                            add(squash(&crate::icons::stem(&p)));
                        }
                    }
                }
                folders.push(canonical(&dir));
            }
        }
        // Programs that run claim their folders and the names along the way:
        // C:\Program Files\Focusrite\Drivers\Notifier.exe claims "Focusrite".
        for program in programs {
            let path = Path::new(program);
            if let Some(parent) = path.parent() {
                folders.push(canonical(&parent.to_string_lossy()));
            }
            let parts: Vec<String> = program.split(['\\', '/']).map(str::to_lowercase).collect();
            if let Some(start) = parts.iter().position(|p| ROOT_NAMES.contains(&p.as_str())) {
                for part in &parts[start + 1..parts.len().saturating_sub(1)] {
                    add(squash(part));
                }
            }
        }
        folders.retain(|f| !f.is_empty());
        Owners { tokens, folders }
    }

    /// True when the name belongs to something installed. Names of four or
    /// more letters also match when one starts with the other ("JetBrains"
    /// and "JetBrains s.r.o.").
    fn claims(&self, name: &str) -> bool {
        let forms = [squash(name), squash(&clean_name(name))];
        if forms[0].len() < 2 {
            return true;
        }
        self.tokens.iter().any(|t| {
            forms.iter().any(|f| {
                t == f || (f.len() >= 4 && t.len() >= 4 && (t.starts_with(f.as_str()) || f.starts_with(t.as_str())))
            })
        })
    }
}

// ---------------------------------------------------------------------------
// Folders and keys that Windows, drivers and tools use without an app entry

/// Folder names in AppData, ProgramData and Program Files that never belong
/// to a removed app.
const NOT_APP_FOLDERS: &[&str] = &[
    "packages",
    "temp",
    "tmp",
    "programs",
    "comms",
    "connecteddevicesplatform",
    "placeholdertilelogofolder",
    "publishers",
    "virtualstore",
    "crashdumps",
    "crashreports",
    "d3dscache",
    "common files",
    "package cache",
    "packagemanagement",
    "regid.1991-06.com.microsoft",
    "ssh",
    "usoprivate",
    "usoshared",
    "softwaredistribution",
    "installshield installation information",
    "uninstall information",
    "internet explorer",
    "reference assemblies",
    "msbuild",
    "dotnet",
    "modifiablewindowsapps",
    "windowsapps",
    "powershell",
    "iis",
    "iis express",
    "nvidia",
    "nvidia corporation",
    "amd",
    "ati",
    "ati technologies",
    "intel",
    "realtek",
    "oem",
    "drivers",
    "chocolatey",
    "npm",
    "npm-cache",
    "pip",
    "pypa",
    "yarn",
    "pnpm",
    "pnpm-cache",
    "nuget",
    "node-gyp",
    "go-build",
    "electron",
    "electron-builder",
    "deno",
    "ms-playwright",
    "cypress",
    "bun",
    "composer",
    "gradle",
    "jdks",
    "github",
    "squirreltemp",
    "uwiper",
    "diagnostics",
    "elevateddiagnostics",
    "peerdistrepub",
];

/// Top-level registry keys that Windows and shared components own.
const NOT_APP_KEYS: &[&str] = &[
    "classes",
    "clients",
    "policies",
    "registeredapplications",
    "wow6432node",
    "odbc",
    "khronos",
    "intel",
    "nvidia corporation",
    "realtek",
    "amd",
    "ati technologies",
    "partner",
    "oem",
    "defaultuserenvironment",
    "appdatalow",
    "chromium",
    "netscape",
    "syncengines",
    "description",
    "hardware",
    "setup",
    "dotnet",
    "openssh",
];

fn is_system_name(name: &str) -> bool {
    let n = name.to_lowercase();
    n.starts_with("microsoft")
        || n.starts_with("windows")
        || n.starts_with('{')
        || n.starts_with('.')
        || n.starts_with('$')
}

// ---------------------------------------------------------------------------
// Shortcut files

/// The target path of a `.lnk` file, read from its LinkInfo block
/// ([MS-SHLLINK]). `None` when the link has none (network links, Store app
/// and installer "advertised" shortcuts) or when it can't be read reliably.
pub fn shortcut_target(bytes: &[u8]) -> Option<String> {
    let u32_at = |at: usize| {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
    };
    let u16_at = |at: usize| bytes.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize);
    if u32_at(0)? != 0x4C {
        return None;
    }
    let flags = u32_at(0x14)?;
    let mut at = 0x4C;
    if flags & 0x1 != 0 {
        at += 2 + u16_at(at)?;
    }
    if flags & 0x2 == 0 {
        return None;
    }
    let info = at;
    let header_size = u32_at(info + 4)?;
    let info_flags = u32_at(info + 8)?;
    if info_flags & 0x1 == 0 {
        return None;
    }
    let utf16_at = |start: usize| {
        let units: Vec<u16> = bytes
            .get(start..)?
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&u| u != 0)
            .collect();
        String::from_utf16(&units).ok()
    };
    // Plain ASCII only: other bytes depend on the PC's code page.
    let ansi_at = |start: usize| {
        let raw: Vec<u8> = bytes.get(start..)?.iter().copied().take_while(|&b| b != 0).collect();
        raw.is_ascii().then(|| String::from_utf8(raw).ok()).flatten()
    };
    let (base, suffix) = if header_size >= 0x24 {
        (
            utf16_at(info + u32_at(info + 0x1C)?)?,
            utf16_at(info + u32_at(info + 0x20)?).unwrap_or_default(),
        )
    } else {
        (
            ansi_at(info + u32_at(info + 0x10)?)?,
            ansi_at(info + u32_at(info + 0x18)?).unwrap_or_default(),
        )
    };
    let target = format!("{base}{suffix}");
    (!target.is_empty()).then_some(target)
}

/// `Some(true)` when a local path is gone. `None` when it can't be told:
/// not a drive path, or the drive itself isn't there (an unplugged disk).
fn is_gone(path: &str) -> Option<bool> {
    let path = expand_env(path.trim().trim_matches('"'));
    let bytes = path.as_bytes();
    if bytes.len() < 3 || bytes[1] != b':' || bytes[2] != b'\\' || !Path::new(&path[..3]).exists() {
        return None;
    }
    Some(!Path::new(&path).exists())
}

// ---------------------------------------------------------------------------
// Scanning

pub fn scan(settings: &Settings) -> Result<OrphanScan> {
    let apps = apps::list_plain()?;
    let startup = crate::startup::list().unwrap_or_default();
    let services = crate::services::list().unwrap_or_default();
    let (tasks, needs_admin) = match crate::tasks::list() {
        Ok(tasks) => (tasks, false),
        Err(Error::NeedsAdmin) => (Vec::new(), true),
        Err(_) => (Vec::new(), false),
    };

    // Programs that are there and run: their folders are in use.
    let mut programs: Vec<String> = Vec::new();
    programs.extend(
        startup
            .iter()
            .filter(|i| i.file_exists == Some(true))
            .filter_map(|i| i.target.clone()),
    );
    programs.extend(
        services
            .iter()
            .filter(|s| s.file_exists == Some(true))
            .filter_map(|s| s.binary.clone()),
    );
    programs.extend(
        tasks
            .iter()
            .filter(|t| t.file_exists == Some(true))
            .filter_map(|t| t.binary.clone()),
    );
    let owners = Owners::new(&apps, &programs);
    let mut found = Found::new(Guard::new(settings), owners.folders.clone(), false);

    // Apps listed as installed whose uninstaller and folder are both gone.
    for app in &apps {
        if app_is_gone(app) {
            if let Some(key) = apps::uninstall_key_path(&app.id) {
                let reason = format!(
                    "“{}” is listed as installed, but its uninstaller and folder are gone",
                    app.name
                );
                found.add("app", Target::RegistryKey { key }, &reason, "high");
            }
        }
    }

    for item in startup.into_iter().filter(|i| i.file_exists == Some(false)) {
        let reason = format!("“{}” starts a program that no longer exists", item.name);
        found.add("startup", item.removal, &reason, "high");
    }
    for service in services
        .into_iter()
        .filter(|s| s.file_exists == Some(false) && !s.is_windows)
    {
        let reason = format!(
            "Service “{}” runs a program that no longer exists",
            service.display_name
        );
        found.add("service", service.removal, &reason, "high");
    }
    for task in tasks
        .into_iter()
        .filter(|t| t.file_exists == Some(false) && !t.is_windows)
    {
        found.add("task", task.removal, "Runs a program that no longer exists", "high");
    }

    // Shortcuts to things that are gone, and Start menu folders left empty.
    for (dir, depth) in shortcut_dirs() {
        let walker = walkdir::WalkDir::new(&dir)
            .min_depth(1)
            .max_depth(depth + 1)
            .follow_links(false);
        for entry in walker.into_iter().flatten() {
            let path = entry.path();
            if entry.file_type().is_dir() {
                let empty = std::fs::read_dir(path).is_ok_and(|mut it| it.next().is_none());
                if empty && !is_system_name(&file_name(path)) {
                    found.add_path(path, "Empty Start menu folder", "medium");
                }
                continue;
            }
            if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk")) {
                continue;
            }
            let target = std::fs::read(path)
                .ok()
                .filter(|b| b.len() < 1 << 20)
                .and_then(|b| shortcut_target(&b));
            if let Some(target) = target.filter(|t| is_gone(t) == Some(true)) {
                found.add_path(path, &format!("Points to {target}, which no longer exists"), "high");
            }
        }
    }

    // PATH entries and App Paths that point nowhere.
    for (scope, entry) in path_entries() {
        if is_gone(&entry) == Some(true) {
            let reason = if scope == "machine" {
                "In the system PATH, but the folder no longer exists"
            } else {
                "In your PATH, but the folder no longer exists"
            };
            found.add(
                "environment",
                Target::PathEntry {
                    scope: scope.into(),
                    entry,
                },
                reason,
                "high",
            );
        }
    }
    for (key, exe) in imp::app_paths() {
        if is_gone(&exe) == Some(true) {
            let reason = format!("Windows still lists {exe} to start by name, but it's gone");
            found.add("registry", Target::RegistryKey { key }, &reason, "high");
        }
    }

    // Data and program folders no installed app claims.
    let roots = [
        ("%APPDATA%", false),
        ("%LOCALAPPDATA%", false),
        (r"%USERPROFILE%\AppData\LocalLow", false),
        (r"%LOCALAPPDATA%\Programs", true),
        ("%ProgramData%", false),
        ("%ProgramFiles%", true),
        ("%ProgramFiles(x86)%", true),
    ];
    for (root, programs_root) in roots {
        let root = expand_env(root);
        if root.contains('%') {
            continue;
        }
        for dir in subdirs(Path::new(&root)) {
            let name = file_name(&dir);
            let lower = name.to_lowercase();
            if is_system_name(&name) || NOT_APP_FOLDERS.contains(&lower.as_str()) || imp::is_link_or_system(&dir) {
                continue;
            }
            if owners.claims(&name) {
                continue;
            }
            let empty = std::fs::read_dir(&dir).is_ok_and(|mut it| it.next().is_none());
            let (reason, confidence) = match (empty, programs_root) {
                (true, _) => ("Empty folder that no installed app uses", "medium"),
                (false, true) => (
                    "No installed app is registered for this folder – it may be a portable app",
                    "low",
                ),
                (false, false) => ("No installed app matches this folder", "low"),
            };
            found.add_path(&dir, reason, confidence);
        }
    }

    // Settings in the registry no installed app claims.
    for software in [r"HKCU\Software", r"HKLM\SOFTWARE", r"HKLM\SOFTWARE\WOW6432Node"] {
        for (name, modified, empty) in imp::subkeys_with_info(software) {
            let lower = name.to_lowercase();
            if is_system_name(&name) || NOT_APP_KEYS.contains(&lower.as_str()) || owners.claims(&name) {
                continue;
            }
            let (reason, confidence) = if empty {
                ("Empty settings key that no installed app uses", "medium")
            } else {
                ("Settings of an app that isn't installed", "low")
            };
            let key = format!(r"{software}\{name}");
            found.add_dated("registry", Target::RegistryKey { key }, reason, confidence, modified);
        }
    }

    Ok(OrphanScan {
        items: found.finish(),
        needs_admin,
    })
}

/// The app's uninstaller and its folder are both gone. MSI apps are left
/// out: Windows Installer removes them by product code.
fn app_is_gone(app: &InstalledApp) -> bool {
    if app.source == "msi" {
        return false;
    }
    let Some((program, _)) = app.uninstall_string.as_deref().and_then(split_command) else {
        return false;
    };
    let folder_gone = app
        .install_location
        .as_deref()
        .is_none_or(|dir| is_gone(dir) != Some(false));
    is_gone(&program) == Some(true) && folder_gone
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn path_entries() -> Vec<(&'static str, String)> {
        Vec::new()
    }

    pub fn app_paths() -> Vec<(String, String)> {
        Vec::new()
    }

    pub fn subkeys_with_info(_key: &str) -> Vec<(String, Option<u64>, bool)> {
        Vec::new()
    }

    pub fn is_link_or_system(path: &Path) -> bool {
        std::fs::symlink_metadata(path).map_or(true, |m| m.file_type().is_symlink())
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::targets::split_key;
    use std::os::windows::fs::MetadataExt;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY};
    use winreg::RegKey;

    fn open(key: &str) -> Option<RegKey> {
        let (hive, sub) = split_key(key)?;
        let root = RegKey::predef(if hive == "HKLM" {
            HKEY_LOCAL_MACHINE
        } else {
            HKEY_CURRENT_USER
        });
        root.open_subkey_with_flags(sub, KEY_READ | KEY_WOW64_64KEY).ok()
    }

    /// Every entry of the user's and the system PATH, as written.
    pub fn path_entries() -> Vec<(&'static str, String)> {
        let sources = [
            ("user", r"HKCU\Environment"),
            (
                "machine",
                r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
            ),
        ];
        let mut out = Vec::new();
        for (scope, key) in sources {
            let Some(path) = open(key).and_then(|k| k.get_value::<String, _>("Path").ok()) else {
                continue;
            };
            out.extend(
                path.split(';')
                    .map(str::trim)
                    .filter(|e| !e.is_empty())
                    .map(|e| (scope, e.to_string())),
            );
        }
        out
    }

    /// App Paths entries (`App Paths\app.exe` → its exe) for the user and the machine.
    pub fn app_paths() -> Vec<(String, String)> {
        let roots = [
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\App Paths",
            r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths",
            r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\App Paths",
        ];
        let mut out = Vec::new();
        for root in roots {
            let Some(key) = open(root) else { continue };
            for name in key.enum_keys().flatten() {
                let exe = key.open_subkey(&name).and_then(|k| k.get_value::<String, _>("")).ok();
                if let Some(exe) = exe.filter(|e| !e.trim().is_empty()) {
                    out.push((format!(r"{root}\{name}"), exe));
                }
            }
        }
        out
    }

    /// Subkeys with when they last changed and whether they're empty.
    pub fn subkeys_with_info(key: &str) -> Vec<(String, Option<u64>, bool)> {
        let Some(parent) = open(key) else {
            return Vec::new();
        };
        parent
            .enum_keys()
            .flatten()
            .filter_map(|name| {
                let info = parent.open_subkey_with_flags(&name, KEY_READ).ok()?.query_info().ok()?;
                let ft = &*info.last_write_time;
                let ticks = (u64::from(ft.dwHighDateTime) << 32) | u64::from(ft.dwLowDateTime);
                // FILETIME counts 100 ns steps from 1601.
                let modified = (ticks / 10_000_000).checked_sub(11_644_473_600);
                Some((name, modified, info.sub_keys == 0 && info.values == 0))
            })
            .collect()
    }

    /// Links, junctions and folders Windows marks as system ones.
    pub fn is_link_or_system(path: &Path) -> bool {
        const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
        std::fs::symlink_metadata(path).map_or(true, |m| {
            m.file_type().is_symlink() || m.file_attributes() & FILE_ATTRIBUTE_SYSTEM != 0
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal .lnk with a LinkInfo block holding `base`.
    fn lnk(base: &str, unicode: bool) -> Vec<u8> {
        let mut out = vec![0u8; 0x4C];
        out[0] = 0x4C;
        out[0x14] = 0x2; // HasLinkInfo
        let header: u32 = if unicode { 0x24 } else { 0x1C };
        let mut info = vec![0u8; header as usize];
        let mut put = |at: usize, v: u32| info[at..at + 4].copy_from_slice(&v.to_le_bytes());
        put(4, header);
        put(8, 0x1); // VolumeIDAndLocalBasePath
        let mut tail = Vec::new();
        if unicode {
            put(0x1C, header);
            let suffix_at = header + (base.encode_utf16().count() as u32 + 1) * 2;
            put(0x20, suffix_at);
            tail.extend(base.encode_utf16().chain([0, 0]).flat_map(u16::to_le_bytes));
        } else {
            put(0x10, header);
            put(0x18, header + base.len() as u32 + 1);
            tail.extend(base.bytes().chain([0, 0]));
        }
        info.extend(tail);
        let size = info.len() as u32;
        info[0..4].copy_from_slice(&size.to_le_bytes());
        out.extend(info);
        out
    }

    #[test]
    fn reads_shortcut_targets() {
        assert_eq!(
            shortcut_target(&lnk(r"C:\Games\Old\game.exe", false)).as_deref(),
            Some(r"C:\Games\Old\game.exe")
        );
        assert_eq!(
            shortcut_target(&lnk(r"D:\Игры\game.exe", true)).as_deref(),
            Some(r"D:\Игры\game.exe")
        );
        assert_eq!(shortcut_target(b"not a shortcut"), None);
        let mut no_info = lnk(r"C:\x.exe", false);
        no_info[0x14] = 0;
        assert_eq!(shortcut_target(&no_info), None);
    }

    fn owners(names: &[(&str, &str)]) -> Owners {
        Owners {
            tokens: names
                .iter()
                .flat_map(|(n, p)| [squash(&clean_name(n)), clean_publisher(p)])
                .filter(|t| t.len() >= 3)
                .collect(),
            folders: Vec::new(),
        }
    }

    #[test]
    fn installed_apps_claim_their_folders() {
        let o = owners(&[
            ("IntelliJ IDEA 2026.2", "JetBrains s.r.o."),
            ("Discord", "Discord Inc."),
        ]);
        assert!(o.claims("JetBrains"));
        assert!(o.claims("discord"));
        assert!(!o.claims("Ravenswatch"));
        assert!(o.claims("x"), "too short to tell");
    }

    #[test]
    fn running_programs_claim_their_vendor_folder() {
        let o = Owners::new(
            &[],
            &[r"C:\Program Files\Focusrite\Drivers\Focusrite Notifier.exe".to_string()],
        );
        assert!(o.claims("Focusrite"));
        assert!(!o.claims("Program Files"));
    }

    #[test]
    fn tells_gone_from_unknown() {
        assert_eq!(is_gone("relative\\path"), None);
        assert_eq!(is_gone(r"\\server\share\x"), None);
    }
}
