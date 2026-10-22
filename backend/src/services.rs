//! Windows services, read from the registry so the list works without
//! admin rights. A service whose executable is gone is a leftover.
#![cfg_attr(not(windows), allow(dead_code))]

use serde::Serialize;

use crate::error::{blocking, Result};
use crate::targets::Target;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServiceItem {
    pub(crate) name: String,
    pub(crate) display_name: String,
    pub(crate) binary: Option<String>,
    pub(crate) file_exists: Option<bool>,
    start_type: &'static str,
    delayed: bool,
    pub(crate) is_windows: bool,
    pub(crate) removal: Target,
}

#[tauri::command]
pub async fn list_services() -> Result<Vec<ServiceItem>> {
    blocking(list).await
}

pub fn list() -> Result<Vec<ServiceItem>> {
    imp::list()
}

fn start_type(start: u32) -> &'static str {
    match start {
        0 => "boot",
        1 => "system",
        2 => "automatic",
        4 => "disabled",
        _ => "manual",
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    use crate::error::Error;

    pub fn list() -> Result<Vec<ServiceItem>> {
        Err(Error::Unsupported)
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::util::{command_executable, expand_env, file_state, is_windows_path, normalize_system_path};
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    // SERVICE_WIN32_OWN_PROCESS | SERVICE_WIN32_SHARE_PROCESS
    const WIN32_SERVICE: u32 = 0x30;
    // Per-user service templates (0x50, 0x60)
    const USER_SERVICE: u32 = 0x40;

    pub fn list() -> Result<Vec<ServiceItem>> {
        let root = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Services", KEY_READ)?;
        let mut out = Vec::new();
        for name in root.enum_keys().flatten() {
            let Ok(key) = root.open_subkey_with_flags(&name, KEY_READ) else {
                continue;
            };
            let kind: u32 = key.get_value("Type").unwrap_or(0);
            if kind & WIN32_SERVICE == 0 || kind & USER_SERVICE != 0 {
                continue;
            }
            // svchost-hosted services name their real binary in Parameters\ServiceDll.
            let service_dll = key
                .open_subkey_with_flags("Parameters", KEY_READ)
                .and_then(|p| p.get_value::<String, _>("ServiceDll"))
                .ok();
            let image = key.get_value::<String, _>("ImagePath").ok();
            let binary = service_dll
                .map(|dll| normalize_system_path(&expand_env(&dll)))
                .or_else(|| {
                    image
                        .as_deref()
                        .and_then(command_executable)
                        .map(|exe| normalize_system_path(&exe))
                });
            let display_name = key
                .get_value::<String, _>("DisplayName")
                .ok()
                .filter(|d| !d.trim().is_empty() && !d.starts_with('@'))
                .unwrap_or_else(|| name.clone());
            out.push(ServiceItem {
                file_exists: binary.as_deref().and_then(file_state),
                is_windows: binary.as_deref().is_none_or(is_windows_path),
                binary,
                start_type: start_type(key.get_value("Start").unwrap_or(3)),
                delayed: key.get_value::<u32, _>("DelayedAutostart").unwrap_or(0) == 1,
                display_name,
                removal: Target::Service { name: name.clone() },
                name,
            });
        }
        out.sort_by_key(|s| s.display_name.to_lowercase());
        Ok(out)
    }
}
