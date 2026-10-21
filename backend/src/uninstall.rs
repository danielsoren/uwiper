//! Running an app's own uninstaller, with an optional restore point first.
//! The leftover scan that follows lives in leftovers.rs.
#![cfg_attr(not(windows), allow(dead_code))]

use serde::Serialize;

use crate::apps;
use crate::error::{blocking, Error, Result};

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RestorePoint {
    created: bool,
    message: String,
}

/// Creates a System Restore point. Never fails: when Windows won't make one,
/// the message says why and the uninstall goes on.
#[tauri::command]
pub async fn create_restore_point(app_name: String) -> Result<RestorePoint> {
    blocking(move || Ok(imp::restore_point(&app_name))).await
}

/// Starts the app's uninstaller and returns right away. Uninstallers often
/// hand over to a second process, so the UI watches `uninstall_finished`.
#[tauri::command]
pub async fn start_uninstall(id: String, quiet: bool) -> Result<()> {
    blocking(move || {
        let app = apps::find(&id)?.ok_or(Error::Other("This app is no longer installed".into()))?;
        let (program, args) = command(&app, quiet)?;
        imp::launch(&program, &args)
    })
    .await
}

/// True once the app's uninstall entry is gone.
#[tauri::command]
pub async fn uninstall_finished(id: String) -> Result<bool> {
    blocking(move || Ok(apps::find(&id)?.is_none())).await
}

/// The program and arguments that remove the app.
fn command(app: &apps::InstalledApp, quiet: bool) -> Result<(String, String)> {
    if let Some(code) = apps::msi_product_code(app) {
        // Basic UI keeps a progress bar but asks nothing.
        let args = if quiet {
            format!("/x {code} /qb")
        } else {
            format!("/x {code}")
        };
        let msiexec = crate::util::system32("msiexec.exe").to_string_lossy().into_owned();
        return Ok((msiexec, args));
    }
    let line = if quiet {
        app.quiet_uninstall_string.as_ref().or(app.uninstall_string.as_ref())
    } else {
        app.uninstall_string.as_ref().or(app.quiet_uninstall_string.as_ref())
    };
    let (program, args) = line
        .and_then(|l| crate::util::split_command(l))
        .ok_or(Error::Other("This app doesn't say how to uninstall it".into()))?;
    // A bare name like "rundll32.exe" is looked up in Windows' folders, never
    // in UWiper's own folder, where something else could stand in for it.
    let program = if program.contains(['\\', '/']) {
        program
    } else {
        crate::util::resolve_executable(&program).ok_or(Error::Other(format!("Can't find {program}")))?
    };
    Ok((program, args))
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn restore_point(_app_name: &str) -> RestorePoint {
        RestorePoint {
            created: false,
            message: "Restore points need Windows".into(),
        }
    }

    pub fn launch(_program: &str, _args: &str) -> Result<()> {
        Err(Error::Unsupported)
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::process::CommandExt;

    pub fn restore_point(app_name: &str) -> RestorePoint {
        if !crate::system::is_elevated() {
            return RestorePoint {
                created: false,
                message: "Skipped – needs administrator rights".into(),
            };
        }
        // The name travels in an environment variable, so nothing in it can
        // be read as part of the script.
        let description = format!("UWiper: before removing {app_name}");
        let script = "Checkpoint-Computer -Description $env:UWIPER_RESTORE_POINT \
                      -RestorePointType APPLICATION_UNINSTALL -WarningAction Stop -ErrorAction Stop";
        let args = ["-NoProfile", "-NonInteractive", "-Command", script];
        let env = [("UWIPER_RESTORE_POINT", description.as_str())];
        match crate::util::run_tool_with_env(r"WindowsPowerShell\v1.0\powershell.exe", &args, &env) {
            Ok(_) => RestorePoint {
                created: true,
                message: "Restore point created".into(),
            },
            Err(e) => {
                let text = e.to_string();
                let message = if text.contains("1440") || text.contains("past") {
                    "Skipped – Windows already made a restore point in the last 24 hours".to_string()
                } else if text.to_lowercase().contains("disabled") || text.to_lowercase().contains("turned off") {
                    "Skipped – System Restore is turned off on this PC".to_string()
                } else {
                    format!(
                        "Skipped – {}",
                        text.lines().next().unwrap_or("Windows didn't create one")
                    )
                };
                RestorePoint {
                    created: false,
                    message,
                }
            }
        }
    }

    pub fn launch(program: &str, args: &str) -> Result<()> {
        let mut command = std::process::Command::new(program);
        if !args.is_empty() {
            command.raw_arg(args);
        }
        match command.spawn() {
            Ok(_) => Ok(()),
            // ERROR_ELEVATION_REQUIRED: the uninstaller asks for admin rights.
            Err(e) if e.raw_os_error() == Some(740) => crate::system::run_elevated(program.as_ref(), args),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Error::Other(
                "The uninstaller is missing – you can still remove what's left".into(),
            )),
            Err(e) => Err(e.into()),
        }
    }
}
