//! Windows services, read from the registry so the list works without
//! admin rights. A service whose executable is gone is a leftover.
#![cfg_attr(not(windows), allow(dead_code))]

use serde::Serialize;
use tauri::State;

use crate::error::{blocking, Error, Result};
use crate::quarantine::settings_of;
use crate::targets::Target;
use crate::{log, AppState};

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

/// Sets how a service starts: "automatic", "delayed", "manual" or "disabled".
#[tauri::command]
pub async fn set_service_start(state: State<'_, AppState>, name: String, start: String) -> Result<()> {
    let settings = settings_of(&state);
    blocking(move || {
        let label = match start.as_str() {
            "automatic" => "Automatic",
            "delayed" => "Automatic (delayed)",
            "manual" => "Manual",
            "disabled" => "Disabled",
            _ => return Err(Error::Other("Unknown start type".into())),
        };
        let item = list()?
            .into_iter()
            .find(|s| s.name.eq_ignore_ascii_case(&name))
            .ok_or(Error::Other("This service is gone – scan again".into()))?;
        if item.is_windows {
            return Err(Error::Other("Part of Windows – UWiper leaves it as is".into()));
        }
        if !crate::system::is_elevated() {
            return Err(Error::NeedsAdmin);
        }
        imp::set_start(&item.name, &start)?;
        log::write(
            &settings,
            &[format!(
                "Service «{}» ({}) set to {label}",
                item.display_name, item.name
            )],
        );
        Ok(())
    })
    .await
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

    pub fn set_start(_name: &str, _start: &str) -> Result<()> {
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

    /// Through the Service Control Manager, so the change applies at once.
    pub fn set_start(name: &str, start: &str) -> Result<()> {
        use windows_sys::Win32::System::Services::{
            ChangeServiceConfig2W, ChangeServiceConfigW, CloseServiceHandle, OpenSCManagerW, OpenServiceW,
            SC_MANAGER_CONNECT, SERVICE_AUTO_START, SERVICE_CHANGE_CONFIG, SERVICE_CONFIG_DELAYED_AUTO_START_INFO,
            SERVICE_DELAYED_AUTO_START_INFO, SERVICE_DEMAND_START, SERVICE_DISABLED, SERVICE_NO_CHANGE,
        };
        let (start_type, delayed) = match start {
            "automatic" => (SERVICE_AUTO_START, false),
            "delayed" => (SERVICE_AUTO_START, true),
            "manual" => (SERVICE_DEMAND_START, false),
            _ => (SERVICE_DISABLED, false),
        };
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let failed = || {
            let e = std::io::Error::last_os_error();
            match e.raw_os_error() {
                Some(5) => Error::NeedsAdmin,
                _ => Error::Io(e),
            }
        };
        unsafe {
            let manager = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
            if manager.is_null() {
                return Err(failed());
            }
            let service = OpenServiceW(manager, wide.as_ptr(), SERVICE_CHANGE_CONFIG);
            if service.is_null() {
                let e = failed();
                CloseServiceHandle(manager);
                return Err(e);
            }
            let mut ok = ChangeServiceConfigW(
                service,
                SERVICE_NO_CHANGE,
                start_type,
                SERVICE_NO_CHANGE,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
            ) != 0;
            if ok && start_type == SERVICE_AUTO_START {
                let info = SERVICE_DELAYED_AUTO_START_INFO {
                    fDelayedAutostart: i32::from(delayed),
                };
                ok = ChangeServiceConfig2W(
                    service,
                    SERVICE_CONFIG_DELAYED_AUTO_START_INFO,
                    &info as *const SERVICE_DELAYED_AUTO_START_INFO as *const core::ffi::c_void,
                ) != 0;
            }
            let e = (!ok).then(failed);
            CloseServiceHandle(service);
            CloseServiceHandle(manager);
            e.map_or(Ok(()), Err)
        }
    }

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
