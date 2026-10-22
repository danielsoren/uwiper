//! Programs that start with Windows: Run keys and Startup folders, with the
//! enabled/disabled state Task Manager keeps under StartupApproved.
#![cfg_attr(not(windows), allow(dead_code))]

use serde::Serialize;
use tauri::State;

use crate::error::{blocking, Error, Result};
use crate::quarantine::settings_of;
use crate::targets::Target;
use crate::{log, AppState};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StartupItem {
    id: String,
    pub(crate) name: String,
    command: String,
    location: String,
    /// The program it starts.
    pub(crate) target: Option<String>,
    pub(crate) file_exists: Option<bool>,
    enabled: bool,
    pub(crate) is_windows: bool,
    /// Machine-wide entries need administrator rights to switch.
    needs_admin: bool,
    /// What to remove to take it out of startup.
    pub(crate) removal: Target,
    /// Where its on/off state lives: machine-wide or not, the StartupApproved
    /// subkey and the value name.
    #[serde(skip)]
    switch: (bool, &'static str, String),
}

#[tauri::command]
pub async fn list_startup() -> Result<Vec<StartupItem>> {
    blocking(list).await
}

pub fn list() -> Result<Vec<StartupItem>> {
    imp::list()
}

/// Turns an entry on or off the way Task Manager does, leaving the entry itself as it is.
#[tauri::command]
pub async fn set_startup_enabled(state: State<'_, AppState>, id: String, enabled: bool) -> Result<()> {
    let settings = settings_of(&state);
    blocking(move || {
        let item = list()?
            .into_iter()
            .find(|i| i.id == id)
            .ok_or(Error::Other("This entry is gone – scan again".into()))?;
        if item.is_windows {
            return Err(Error::Other("Part of Windows – UWiper leaves it as is".into()));
        }
        imp::set_enabled(&item, enabled)?;
        let state = if enabled { "turned on" } else { "turned off" };
        log::write(
            &settings,
            &[format!("Startup entry «{}» {state} ({})", item.name, item.location)],
        );
        Ok(())
    })
    .await
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    use crate::error::Error;

    pub fn list() -> Result<Vec<StartupItem>> {
        Err(Error::Unsupported)
    }

    pub fn set_enabled(_item: &StartupItem, _enabled: bool) -> Result<()> {
        Err(Error::Unsupported)
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::util::{
        expand_env, is_windows_component, now_secs, program_state, resolve_executable, uses_per_user_vars,
    };
    use winreg::enums::{RegType, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};
    use winreg::{RegKey, RegValue};

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
            false,
            "Startup folder",
        );
        folder(
            &mut out,
            &expand_env(r"%ProgramData%\Microsoft\Windows\Start Menu\Programs\StartUp"),
            true,
            "Startup folder (all users)",
        );
        out.sort_by_key(|i| i.name.to_lowercase());
        Ok(out)
    }

    /// Writes what Task Manager writes: 02 for on; 03 and the time for off.
    pub fn set_enabled(item: &StartupItem, enabled: bool) -> Result<()> {
        let (machine, sub, name) = &item.switch;
        let root = RegKey::predef(if *machine {
            HKEY_LOCAL_MACHINE
        } else {
            HKEY_CURRENT_USER
        });
        let denied = |e: std::io::Error| match e.kind() {
            std::io::ErrorKind::PermissionDenied => Error::NeedsAdmin,
            _ => Error::Io(e),
        };
        let (key, _) = root.create_subkey(format!(r"{APPROVED}\{sub}")).map_err(denied)?;
        // FILETIME: 100 ns steps since 1601.
        let stamp = if enabled {
            0
        } else {
            (now_secs() + 11_644_473_600) * 10_000_000
        };
        let mut bytes = vec![if enabled { 2 } else { 3 }, 0, 0, 0];
        bytes.extend_from_slice(&stamp.to_le_bytes());
        key.set_raw_value(
            name,
            &RegValue {
                bytes: bytes.into(),
                vtype: RegType::REG_BINARY,
            },
        )
        .map_err(denied)
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
        approved_sub: &'static str,
        location: &str,
        key_path: &str,
    ) {
        let machine = key_path.starts_with("HKLM");
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
                is_windows: target.as_deref().is_some_and(is_windows_component),
                // Machine-wide entries run for every user: a per-user path
                // missing for this user may be there for another.
                file_exists: if machine && uses_per_user_vars(&command) {
                    None
                } else {
                    target.as_deref().and_then(program_state)
                },
                target,
                removal: Target::RegistryValue {
                    key: key_path.to_string(),
                    name: name.clone(),
                },
                needs_admin: machine,
                switch: (machine, approved_sub, name.clone()),
                name,
                command,
                location: location.to_string(),
            });
        }
    }

    fn folder(out: &mut Vec<StartupItem>, dir: &str, machine: bool, location: &str) {
        let approved_root = RegKey::predef(if machine { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER });
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
                enabled: approved(&approved_root, "StartupFolder", &file),
                needs_admin: machine,
                switch: (machine, "StartupFolder", file.clone()),
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
