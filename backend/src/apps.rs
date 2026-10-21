//! Installed programs, read from the same Uninstall keys that Windows
//! Settings uses (machine-wide 64/32-bit and per-user).
#![cfg_attr(not(windows), allow(dead_code))]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{blocking, Result};
use crate::icons;
use crate::util::{command_executable, dir_size, is_windows_path, parallel_map};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    id: String,
    name: String,
    publisher: Option<String>,
    version: Option<String>,
    install_location: Option<String>,
    /// ISO date, e.g. "2026-03-12"
    install_date: Option<String>,
    /// The best estimate: the measured folder or what the installer
    /// reported, whichever is larger (files can live outside the folder).
    size_bytes: Option<u64>,
    /// What the installer reported in the registry.
    reported_bytes: Option<u64>,
    /// The install folder measured on disk.
    folder_bytes: Option<u64>,
    /// The app's icon as a PNG data URL.
    icon: Option<String>,
    /// "win32" or "msi"
    source: &'static str,
    /// "machine" or "user"
    scope: &'static str,
    arch: Option<&'static str>,
    uninstall_string: Option<String>,
    quiet_uninstall_string: Option<String>,
    registry_key: String,
    #[serde(skip)]
    display_icon: Option<String>,
}

/// Lists installed apps with their real size and icon, so nothing changes
/// once the list is on screen.
#[tauri::command]
pub async fn list_apps() -> Result<Vec<InstalledApp>> {
    blocking(|| {
        let mut apps = imp::list()?;
        measure_and_find_icons(&mut apps);
        Ok(apps)
    })
    .await
}

fn measure_and_find_icons(apps: &mut [InstalledApp]) {
    let shared = shared_folders();
    for app in apps.iter_mut() {
        if app.install_location.is_none() {
            app.install_location = location_from_uninstaller(app.uninstall_string.as_deref(), &shared);
        }
    }

    // Apps can share a folder; walk each one once.
    let mut folders: HashMap<String, String> = HashMap::new();
    for path in apps.iter().filter_map(|a| a.install_location.as_deref()) {
        if is_own_folder(path, &shared) {
            folders.entry(normalize(path)).or_insert_with(|| path.to_string());
        }
    }
    let folders: Vec<(String, String)> = folders.into_iter().collect();
    let sizes: HashMap<&str, u64> = folders
        .iter()
        .map(|(key, _)| key.as_str())
        .zip(parallel_map(&folders, |(_, path)| dir_size(Path::new(path)).0))
        .collect();

    let found = parallel_map(apps, icon_of);
    for (app, icon) in apps.iter_mut().zip(found) {
        app.folder_bytes = app
            .install_location
            .as_deref()
            .and_then(|p| sizes.get(normalize(p).as_str()).copied())
            .filter(|&b| b > 0);
        app.size_bytes = app.folder_bytes.max(app.reported_bytes);
        app.icon = icon;
    }
}

fn normalize(path: &str) -> String {
    path.trim().trim_end_matches(['\\', '/']).to_lowercase()
}

/// Folders that hold many apps, or none in particular. An app that reports
/// one of these as its location would be measured as everything inside it.
fn shared_folders() -> Vec<String> {
    let mut out: Vec<String> = [
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramW6432",
        "ProgramData",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "PUBLIC",
    ]
    .iter()
    .filter_map(|var| std::env::var(var).ok())
    .map(|p| normalize(&p))
    .collect();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        out.push(normalize(&format!(r"{local}\Programs")));
    }
    if let Ok(drive) = std::env::var("SystemDrive") {
        out.push(normalize(&format!(r"{drive}\Users")));
    }
    out
}

/// True when `path` looks like the app's own folder and is fine to measure.
fn is_own_folder(path: &str, shared: &[String]) -> bool {
    const NOT_APP_FOLDERS: [&str; 5] = [
        r"\common files",
        r"\installshield installation information",
        r"\package cache",
        r"\windows\installer",
        r"\temp",
    ];
    let p = normalize(path);
    let below_root = p.split_once('\\').is_some_and(|(_, rest)| !rest.is_empty());
    below_root && !is_windows_path(&p) && !shared.contains(&p) && !NOT_APP_FOLDERS.iter().any(|m| p.contains(m))
}

/// Many installers (Inno Setup, NSIS) put the uninstaller in the app's folder
/// but don't record the folder itself.
fn location_from_uninstaller(uninstall: Option<&str>, shared: &[String]) -> Option<String> {
    let exe = PathBuf::from(command_executable(uninstall?)?);
    if !icons::is_uninstaller(&exe) {
        return None;
    }
    let dir = exe.parent()?.to_string_lossy().into_owned();
    (is_own_folder(&dir, shared) && Path::new(&dir).is_dir()).then_some(dir)
}

fn icon_of(app: &InstalledApp) -> Option<String> {
    let listed = app
        .display_icon
        .as_deref()
        .and_then(icons::parse_spec)
        .filter(|(file, _)| !icons::is_uninstaller(file) && file.is_file());
    if let Some((file, index)) = listed {
        if let Some(url) = icons::data_url(&file, index) {
            return Some(url);
        }
    }
    let exe = main_exe(Path::new(app.install_location.as_deref()?), &app.name)?;
    icons::data_url(&exe, 0)
}

/// Guesses the app's own exe among the ones at the top of its folder.
fn main_exe(dir: &Path, name: &str) -> Option<PathBuf> {
    let exes: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("exe")) && !icons::is_helper_exe(p))
        .collect();
    pick_main_exe(exes, name)
}

fn pick_main_exe(mut exes: Vec<PathBuf>, name: &str) -> Option<PathBuf> {
    let squash = |s: &str| {
        s.chars()
            .filter(char::is_ascii_alphanumeric)
            .collect::<String>()
            .to_lowercase()
    };
    let app = squash(name);
    let first_word = squash(name.split_whitespace().next().unwrap_or_default());
    let matches = |p: &PathBuf| {
        let stem = squash(&icons::stem(p));
        stem.len() >= 3 && (app.contains(&stem) || (first_word.len() >= 3 && stem.contains(&first_word)))
    };
    if let Some(found) = exes.iter().find(|p| matches(p)) {
        return Some(found.clone());
    }
    (exes.len() == 1).then(|| exes.remove(0))
}

/// `20260312` → `2026-03-12`
fn iso_date(raw: &str) -> Option<String> {
    let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
    if digits.len() != 8 {
        return None;
    }
    Some(format!("{}-{}-{}", &digits[..4], &digits[4..6], &digits[6..]))
}

fn is_update(release_type: &str) -> bool {
    matches!(release_type, "Update" | "Hotfix" | "Security Update" | "Service Pack")
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    use crate::error::Error;

    pub fn list() -> Result<Vec<InstalledApp>> {
        Err(Error::Unsupported)
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::collections::HashSet;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};
    use winreg::RegKey;

    const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";

    pub fn list() -> Result<Vec<InstalledApp>> {
        let sources = [
            (HKEY_LOCAL_MACHINE, "HKLM", KEY_WOW64_64KEY, "machine", Some("x64")),
            (HKEY_LOCAL_MACHINE, "HKLM", KEY_WOW64_32KEY, "machine", Some("x86")),
            (HKEY_CURRENT_USER, "HKCU", 0, "user", None),
        ];
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for (hive, hive_name, view, scope, arch) in sources {
            let Ok(root) = RegKey::predef(hive).open_subkey_with_flags(UNINSTALL, KEY_READ | view) else {
                continue;
            };
            for key_name in root.enum_keys().flatten() {
                let Ok(key) = root.open_subkey_with_flags(&key_name, KEY_READ | view) else {
                    continue;
                };
                let Some(app) = read(&key, &key_name, hive_name, view, scope, arch) else {
                    continue;
                };
                let dedupe = (app.name.to_lowercase(), app.version.clone(), app.publisher.clone());
                if seen.insert(dedupe) {
                    out.push(app);
                }
            }
        }
        out.sort_by_key(|a| a.name.to_lowercase());
        Ok(out)
    }

    fn read(
        key: &RegKey,
        key_name: &str,
        hive: &str,
        view: u32,
        scope: &'static str,
        arch: Option<&'static str>,
    ) -> Option<InstalledApp> {
        let text = |name: &str| {
            key.get_value::<String, _>(name)
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        };
        let dword = |name: &str| key.get_value::<u32, _>(name).ok();

        let name = text("DisplayName")?;
        // Hidden components, updates and patches are not separate programs.
        if dword("SystemComponent") == Some(1)
            || text("ParentKeyName").is_some()
            || text("ReleaseType").is_some_and(|t| is_update(&t))
        {
            return None;
        }
        let uninstall_string = text("UninstallString");
        let quiet_uninstall_string = text("QuietUninstallString");
        if uninstall_string.is_none() && quiet_uninstall_string.is_none() {
            return None;
        }
        let view_suffix = if view == KEY_WOW64_32KEY { " (32-bit view)" } else { "" };
        Some(InstalledApp {
            id: format!("{hive}|{view}|{key_name}"),
            publisher: text("Publisher"),
            version: text("DisplayVersion"),
            install_location: text("InstallLocation").map(|p| p.trim_matches('"').trim_end_matches('\\').to_string()),
            install_date: text("InstallDate").as_deref().and_then(iso_date),
            size_bytes: None,
            reported_bytes: dword("EstimatedSize")
                .filter(|&kb| kb > 0)
                .map(|kb| u64::from(kb) * 1024),
            folder_bytes: None,
            icon: None,
            display_icon: text("DisplayIcon"),
            source: if dword("WindowsInstaller") == Some(1) {
                "msi"
            } else {
                "win32"
            },
            scope,
            arch,
            uninstall_string,
            quiet_uninstall_string,
            registry_key: format!(r"{hive}\{UNINSTALL}\{key_name}{view_suffix}"),
            name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_install_dates() {
        assert_eq!(iso_date("20260312").as_deref(), Some("2026-03-12"));
        assert_eq!(iso_date("2026-03-12").as_deref(), Some("2026-03-12"));
        assert_eq!(iso_date("3/12/2026"), None);
    }

    #[test]
    fn measures_only_an_apps_own_folder() {
        let shared = vec![normalize(r"C:\Program Files"), normalize(r"C:\Users\me\AppData\Local")];
        assert!(is_own_folder(r"C:\Program Files\Ravenswatch\", &shared));
        assert!(is_own_folder(r"D:\Games\Ravenswatch", &shared));
        assert!(is_own_folder(r"C:\Python312", &shared));
        assert!(!is_own_folder(r"C:\Program Files", &shared));
        assert!(!is_own_folder(r"C:\Program Files\", &shared));
        assert!(!is_own_folder(r"C:\Users\me\AppData\Local", &shared));
        assert!(!is_own_folder(r"D:\", &shared));
        assert!(!is_own_folder(r"C:\Program Files (x86)\Common Files\Vendor", &shared));
        assert!(!is_own_folder(r"C:\ProgramData\Package Cache\{1234}", &shared));
    }

    #[test]
    fn picks_the_apps_own_exe() {
        let exes = |names: &[&str]| {
            names
                .iter()
                .map(|n| PathBuf::from(format!(r"C:\App\{n}")))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            pick_main_exe(exes(&["helper.exe", "Ravenswatch.exe"]), "Ravenswatch"),
            Some(PathBuf::from(r"C:\App\Ravenswatch.exe"))
        );
        assert_eq!(
            pick_main_exe(exes(&["Code.exe"]), "Microsoft Visual Studio Code"),
            Some(PathBuf::from(r"C:\App\Code.exe"))
        );
        assert_eq!(
            pick_main_exe(exes(&["obs64.exe"]), "OBS Studio"),
            Some(PathBuf::from(r"C:\App\obs64.exe"))
        );
        assert_eq!(pick_main_exe(exes(&["a.exe", "b.exe"]), "Something"), None);
    }

    #[test]
    fn skips_updates() {
        assert!(is_update("Security Update"));
        assert!(!is_update("Application"));
    }
}
