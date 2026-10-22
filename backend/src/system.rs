//! Window material, elevation, opening folders and the few things UWiper
//! writes outside its own folder.

use serde::Serialize;
use tauri::{AppHandle, State, WebviewWindow};

use crate::error::{Error, Result};
use crate::{paths, AppState};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    elevated: bool,
    windows: bool,
    material: &'static str,
    home: String,
    version: &'static str,
}

#[tauri::command(async)]
pub fn environment(state: State<'_, AppState>) -> Environment {
    Environment {
        elevated: is_elevated(),
        windows: cfg!(windows),
        material: *state.material.lock().unwrap(),
        home: paths::home().to_string_lossy().into_owned(),
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// Mica on Windows 11. Elsewhere the frontend paints an opaque background.
pub fn apply_material(window: &WebviewWindow) -> &'static str {
    #[cfg(windows)]
    if window_vibrancy::apply_mica(window, Some(true)).is_ok() {
        return "mica";
    }
    let _ = window;
    "none"
}

#[cfg(windows)]
pub fn is_elevated() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut TOKEN_ELEVATION as *mut core::ffi::c_void,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    false
}

/// Starts a program with administrator rights (UAC prompt).
#[cfg(windows)]
pub fn run_elevated(program: &std::ffi::OsStr, params: &str) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let wide = |s: &std::ffi::OsStr| s.encode_wide().chain(Some(0)).collect::<Vec<u16>>();
    let (verb, program, params) = (wide("runas".as_ref()), wide(program), wide(params.as_ref()));
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            program.as_ptr(),
            params.as_ptr(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    // Values above 32 mean success; otherwise the UAC prompt was declined.
    if result as isize <= 32 {
        return Err(Error::Other("Administrator rights were not granted".into()));
    }
    Ok(())
}

/// Starts a new elevated copy of UWiper (UAC prompt) and closes this one.
#[tauri::command(async)]
pub fn restart_as_admin(app: AppHandle) -> Result<()> {
    #[cfg(windows)]
    {
        run_elevated(std::env::current_exe()?.as_os_str(), "")?;
        app.exit(0);
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err(Error::Unsupported)
    }
}

/// Opens a folder in Explorer.
#[tauri::command(async)]
pub fn open_folder(path: String) -> Result<()> {
    if !std::path::Path::new(&path).is_dir() {
        return Err(Error::Other("This folder doesn't exist yet".into()));
    }
    #[cfg(windows)]
    {
        std::process::Command::new(std::path::Path::new(&crate::util::windows_dir()).join("explorer.exe"))
            .arg(&path)
            .spawn()?;
        Ok(())
    }
    #[cfg(not(windows))]
    Err(Error::Unsupported)
}

/// Opens Explorer with the file selected.
#[tauri::command(async)]
pub fn reveal_path(path: String) -> Result<()> {
    if !std::path::Path::new(&path).exists() {
        return Err(Error::Other("This file or folder no longer exists".into()));
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new(std::path::Path::new(&crate::util::windows_dir()).join("explorer.exe"))
            .raw_arg(format!("/select,\"{path}\""))
            .spawn()?;
        Ok(())
    }
    #[cfg(not(windows))]
    Err(Error::Unsupported)
}

/// Earlier versions could add UWiper to startup. That option is gone, so
/// an entry pointing at this exe is removed – UWiper keeps nothing outside
/// its folder.
pub fn remove_old_autostart() {
    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
        use winreg::RegKey;

        let run = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            KEY_READ | KEY_SET_VALUE,
        );
        let (Ok(run), Ok(exe)) = (run, std::env::current_exe()) else {
            return;
        };
        let ours = run.get_value::<String, _>("UWiper").is_ok_and(|command| {
            crate::util::command_executable(&command)
                .is_some_and(|p| crate::util::is_within(&p, &exe.to_string_lossy()))
        });
        if ours {
            let _ = run.delete_value("UWiper");
        }
    }
}
