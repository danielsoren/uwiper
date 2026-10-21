//! Programs that start with Windows: Run keys and Startup folders, with the
//! enabled/disabled state Task Manager keeps under StartupApproved.
#![cfg_attr(not(windows), allow(dead_code))]

use serde::Serialize;

use crate::error::{blocking, Result};
use crate::targets::Target;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StartupItem {
    id: String,
    pub(crate) name: String,
    command: String,
    location: String,
    /// The program it starts.
    pub(crate) target: Option<String>,
    file_exists: Option<bool>,
    enabled: bool,
    pub(crate) is_windows: bool,
    /// What to remove to take it out of startup.
    pub(crate) removal: Target,
}

#[tauri::command]
pub async fn list_startup() -> Result<Vec<StartupItem>> {
    blocking(list).await
}

pub fn list() -> Result<Vec<StartupItem>> {
    imp::list()
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    use crate::error::Error;

    pub fn list() -> Result<Vec<StartupItem>> {
        Err(Error::Unsupported)
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::util::{expand_env, file_state, is_windows_path, resolve_executable};
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};
    use winreg::RegKey;

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";

    pub fn list() -> Result<Vec<StartupItem>> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let mut out = Vec::new();
        run_key(&mut out, &hkcu, 0, "Run", r"HKCU\…\Run", &format!(r"HKCU\{RUN}"));
        run_key(
            &mut out,
            &hklm,
            KEY_WOW64_64KEY,
            "Run",
            r"HKLM\…\Run",
            &format!(r"HKLM\{RUN}"),
        );
        let run32 = format!(r"HKLM\{}", RUN.replacen("Software", r"SOFTWARE\WOW6432Node", 1));
        run_key(
            &mut out,
            &hklm,
            KEY_WOW64_32KEY,
            "Run32",
            r"HKLM\…\Run (32-bit)",
            &run32,
        );
        folder(
            &mut out,
            &expand_env(r"%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup"),
            &hkcu,
            "Startup folder",
        );
        folder(
            &mut out,
            &expand_env(r"%ProgramData%\Microsoft\Windows\Start Menu\Programs\StartUp"),
            &hklm,
            "Startup folder (all users)",
        );
        out.sort_by_key(|i| i.name.to_lowercase());
        Ok(out)
    }

    /// Task Manager writes 0x02/0x06 for enabled and 0x03 for disabled.
    fn approved(root: &RegKey, sub: &str, name: &str) -> bool {
        root.open_subkey(format!(r"{APPROVED}\{sub}"))
            .and_then(|key| key.get_raw_value(name))
            .map(|value| value.bytes.first().is_none_or(|b| b % 2 == 0))
            .unwrap_or(true)
    }

    fn run_key(
        out: &mut Vec<StartupItem>,
        root: &RegKey,
        view: u32,
        approved_sub: &str,
        location: &str,
        key_path: &str,
    ) {
        let Ok(key) = root.open_subkey_with_flags(RUN, KEY_READ | view) else {
            return;
        };
        for (name, _) in key.enum_values().flatten() {
            let Ok(command) = key.get_value::<String, _>(&name) else {
                continue;
            };
            if command.trim().is_empty() {
                continue;
            }
            let target = resolve_executable(&command);
            out.push(StartupItem {
                id: format!("{location}|{name}"),
                enabled: approved(root, approved_sub, &name),
                is_windows: target.as_deref().is_some_and(is_windows_path),
                file_exists: target.as_deref().and_then(file_state),
                target,
                removal: Target::RegistryValue {
                    key: key_path.to_string(),
                    name: name.clone(),
                },
                name,
                command,
                location: location.to_string(),
            });
        }
    }

    fn folder(out: &mut Vec<StartupItem>, dir: &str, approved_root: &RegKey, location: &str) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            if file.eq_ignore_ascii_case("desktop.ini") || !entry.path().is_file() {
                continue;
            }
            let path = entry.path().to_string_lossy().into_owned();
            let name = file
                .rsplit_once('.')
                .map_or(file.as_str(), |(stem, _)| stem)
                .to_string();
            out.push(StartupItem {
                id: format!("{location}|{file}"),
                enabled: approved(approved_root, "StartupFolder", &file),
                name,
                command: path.clone(),
                location: location.to_string(),
                removal: Target::Path { path: path.clone() },
                target: Some(path),
                file_exists: Some(true),
                is_windows: false,
            });
        }
    }
}
