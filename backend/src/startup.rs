//! Programs that start with Windows: Run keys, Startup folders and the
//! startup tasks of app packages, with the enabled/disabled state Task
//! Manager keeps for each.
#![cfg_attr(not(windows), allow(dead_code))]

use serde::Serialize;
use tauri::State;

use crate::error::{blocking, Error, Result};
use crate::quarantine::settings_of;
use crate::targets::Target;
use crate::util::is_host;
use crate::{log, AppState};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StartupItem {
    id: String,
    pub(crate) name: String,
    command: String,
    location: String,
    /// Who made it: the program's company, or the app package's publisher.
    publisher: Option<String>,
    /// The program it starts.
    pub(crate) target: Option<String>,
    pub(crate) file_exists: Option<bool>,
    enabled: bool,
    pub(crate) is_windows: bool,
    /// Machine-wide entries need administrator rights to switch.
    needs_admin: bool,
    /// A policy on this PC turns it on or off.
    policy: bool,
    /// What to remove to take it out of startup. An app package's startup
    /// task has nothing of its own: it goes with the app.
    pub(crate) removal: Option<Target>,
    #[serde(skip)]
    switch: Switch,
}

/// Where an entry's on/off state lives.
#[derive(Clone, Debug)]
enum Switch {
    /// A StartupApproved value: machine-wide or not, the subkey and the value name.
    Approved {
        machine: bool,
        sub: &'static str,
        name: String,
    },
    /// The key holding an app package's startup task state.
    Package { key: String },
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
        if item.policy {
            return Err(Error::Other(
                "A policy on this PC sets it – UWiper leaves it as is".into(),
            ));
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

/// Task Manager's name for an entry: the program's description, unless it
/// only repeats the file name (Discord's `Update.exe` says "Update") or the
/// program just runs another file.
fn display_name(entry: &str, program: Option<&str>, description: Option<String>) -> String {
    let stem = program.map(|p| {
        let file = p.rsplit(['\\', '/']).next().unwrap_or(p);
        file.rsplit_once('.').map_or(file, |(stem, _)| stem).to_lowercase()
    });
    match description {
        Some(d) if !program.is_some_and(is_host) && stem.as_deref() != Some(d.to_lowercase().as_str()) => d,
        _ => entry.to_string(),
    }
}

/// A startup task from an app package's manifest.
#[derive(Debug, PartialEq)]
struct PackageTask {
    id: String,
    /// What the manifest asks for before the user or the app changes it.
    enabled: bool,
    /// Names to try in order, possibly `ms-resource:` references.
    names: Vec<String>,
    publisher: Option<String>,
    /// The program, relative to the package folder.
    executable: Option<String>,
}

/// Reads the startup tasks an `AppxManifest.xml` declares.
fn package_tasks(manifest: &str) -> Vec<PackageTask> {
    let Ok(doc) = roxmltree::Document::parse(manifest.trim_start_matches('\u{feff}')) else {
        return Vec::new();
    };
    let is = |n: &roxmltree::Node, tag: &str| n.is_element() && n.tag_name().name() == tag;
    let properties = doc.descendants().find(|n| is(n, "Properties"));
    let property = |tag: &str| {
        properties?
            .children()
            .find(|c| is(c, tag))
            .and_then(|c| c.text())
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
    };
    let mut out = Vec::new();
    for app in doc.descendants().filter(|n| is(n, "Application")) {
        let app_name = app
            .descendants()
            .find(|n| is(n, "VisualElements"))
            .and_then(|v| v.attribute("DisplayName"));
        for extension in app
            .descendants()
            .filter(|n| is(n, "Extension") && n.attribute("Category") == Some("windows.startupTask"))
        {
            let Some(task) = extension.children().find(|c| is(c, "StartupTask")) else {
                continue;
            };
            let Some(id) = task.attribute("TaskId").filter(|t| !t.is_empty()) else {
                continue;
            };
            let names = [task.attribute("DisplayName"), app_name]
                .into_iter()
                .flatten()
                .map(str::to_string)
                .chain(property("DisplayName"))
                .filter(|n| !n.trim().is_empty())
                .collect();
            out.push(PackageTask {
                id: id.to_string(),
                enabled: task.attribute("Enabled") == Some("true"),
                names,
                publisher: property("PublisherDisplayName"),
                executable: extension
                    .attribute("Executable")
                    .or(app.attribute("Executable"))
                    .filter(|e| !e.contains('$'))
                    .map(str::to_string),
            });
        }
    }
    out
}

/// The `ms-resource://` address of a manifest string such as
/// `ms-resource:AppName` in the package `package`.
fn resource_uri(raw: &str, package: &str) -> Option<String> {
    let rest = raw.strip_prefix("ms-resource:")?;
    if let Some(full) = rest.strip_prefix("//").filter(|r| !r.starts_with('/')) {
        return Some(format!("ms-resource://{full}"));
    }
    let rest = rest.trim_start_matches('/');
    Some(if rest.contains('/') {
        format!("ms-resource://{package}/{rest}")
    } else {
        format!("ms-resource://{package}/Resources/{rest}")
    })
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
        expand_env, file_info, hosted_file, indirect_string, is_windows_component, is_windows_program, now_secs,
        program_state, resolve_executable, split_command, uses_per_user_vars,
    };
    use winreg::enums::{RegType, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};
    use winreg::{RegKey, RegValue};

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";
    const APP_MODEL: &str = r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel";

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
        packages(&mut out, &hkcu);
        out.sort_by_key(|i| i.name.to_lowercase());
        Ok(out)
    }

    pub fn set_enabled(item: &StartupItem, enabled: bool) -> Result<()> {
        let denied = |e: std::io::Error| match e.kind() {
            std::io::ErrorKind::PermissionDenied => Error::NeedsAdmin,
            _ => Error::Io(e),
        };
        match &item.switch {
            // Writes what Task Manager writes: 02 for on; 03 and the time for off.
            Switch::Approved { machine, sub, name } => {
                let root = RegKey::predef(if *machine {
                    HKEY_LOCAL_MACHINE
                } else {
                    HKEY_CURRENT_USER
                });
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
            // 2 – enabled, 1 – disabled by the user.
            Switch::Package { key } => {
                let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(key).map_err(denied)?;
                key.set_value("State", &(if enabled { 2u32 } else { 1u32 }))
                    .map_err(denied)
            }
        }
    }

    /// Task Manager writes 0x02/0x06 for enabled and 0x03 for disabled.
    fn approved(root: &RegKey, sub: &str, name: &str) -> bool {
        root.open_subkey(format!(r"{APPROVED}\{sub}"))
            .and_then(|key| key.get_raw_value(name))
            .map(|value| value.bytes.first().is_none_or(|b| b % 2 == 0))
            .unwrap_or(true)
    }

    /// The company of a program, unless it only hosts another file.
    fn company(program: Option<&str>) -> Option<String> {
        program.filter(|p| !is_host(p)).and_then(|p| file_info(p).company)
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
            // rundll32, a shell or a script host: what counts is the file it runs.
            let host = resolve_executable(&command);
            let target = host
                .as_deref()
                .and_then(|h| hosted_file(h, &split_command(&command)?.1))
                .or(host);
            let program = target.as_deref();
            out.push(StartupItem {
                id: format!("{location}|{name}"),
                enabled: approved(root, approved_sub, &name),
                is_windows: program.is_some_and(|p| !is_host(p) && is_windows_program(p)),
                // Machine-wide entries run for every user: a per-user path
                // missing for this user may be there for another.
                file_exists: if machine && uses_per_user_vars(&command) {
                    None
                } else {
                    program.and_then(program_state)
                },
                publisher: company(program),
                removal: Some(Target::RegistryValue {
                    key: key_path.to_string(),
                    name: name.clone(),
                }),
                needs_admin: machine,
                policy: false,
                switch: Switch::Approved {
                    machine,
                    sub: approved_sub,
                    name: name.clone(),
                },
                name: display_name(&name, program, program.and_then(|p| file_info(p).description)),
                target,
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
            // A shortcut starts what it points to; on a drive that isn't
            // there now, whether it's gone can't be told.
            let linked = file
                .to_lowercase()
                .ends_with(".lnk")
                .then(|| std::fs::read(&path).ok())
                .flatten()
                .and_then(|bytes| crate::orphans::shortcut_target(&bytes));
            let file_exists = match &linked {
                Some(t) if t.get(..3).is_some_and(|root| std::path::Path::new(root).exists()) => program_state(t),
                Some(_) => None,
                None => Some(true),
            };
            let target = linked.unwrap_or_else(|| path.clone());
            out.push(StartupItem {
                id: format!("{location}|{file}"),
                enabled: approved(&approved_root, "StartupFolder", &file),
                needs_admin: machine,
                policy: false,
                switch: Switch::Approved {
                    machine,
                    sub: "StartupFolder",
                    name: file.clone(),
                },
                // The shortcut's own name is the one people chose to see.
                name: file
                    .rsplit_once('.')
                    .map_or(file.as_str(), |(stem, _)| stem)
                    .to_string(),
                publisher: company(Some(target.as_str())),
                command: path.clone(),
                location: location.to_string(),
                removal: Some(Target::Path { path }),
                target: Some(target),
                file_exists,
                is_windows: false,
            });
        }
    }

    /// Startup tasks of the app packages installed for this user (Store and
    /// MSIX apps). Their state lives under SystemAppData: 0 – disabled,
    /// 1 – disabled by the user, 2 – enabled, 3 and 4 – disabled or enabled
    /// by policy; with no state yet the manifest's own setting counts.
    fn packages(out: &mut Vec<StartupItem>, hkcu: &RegKey) {
        let Ok(repository) = hkcu.open_subkey(format!(r"{APP_MODEL}\Repository\Packages")) else {
            return;
        };
        for full_name in repository.enum_keys().flatten() {
            let Ok(root) = repository
                .open_subkey(&full_name)
                .and_then(|k| k.get_value::<String, _>("PackageRootFolder"))
            else {
                continue;
            };
            let Ok(manifest) = std::fs::read_to_string(format!(r"{root}\AppxManifest.xml")) else {
                continue;
            };
            if !manifest.contains("windows.startupTask") {
                continue;
            }
            // Name_Version_Architecture_ResourceId_PublisherId
            let parts: Vec<&str> = full_name.split('_').collect();
            let (Some(package), Some(publisher_id)) = (parts.first(), parts.last()) else {
                continue;
            };
            let family = format!("{package}_{publisher_id}");
            let text = |raw: &str| match resource_uri(raw, package) {
                Some(uri) => indirect_string(&format!("@{{{full_name}?{uri}}}")),
                None => Some(raw.to_string()),
            };
            for task in package_tasks(&manifest) {
                let key = format!(r"{APP_MODEL}\SystemAppData\{family}\{}", task.id);
                let state = hkcu.open_subkey(&key).and_then(|k| k.get_value::<u32, _>("State")).ok();
                let target = task.executable.as_ref().map(|e| format!(r"{root}\{e}"));
                let name = task
                    .names
                    .iter()
                    .find_map(|n| text(n))
                    .unwrap_or_else(|| package.rsplit('.').next().unwrap_or(package).to_string());
                out.push(StartupItem {
                    id: format!("Packaged app|{family}|{}", task.id),
                    enabled: state.map_or(task.enabled, |s| s == 2 || s == 4),
                    policy: matches!(state, Some(3 | 4)),
                    is_windows: is_windows_component(&root),
                    // The package keeps its files; Windows looks after them.
                    file_exists: target.as_deref().and_then(program_state).filter(|&there| there),
                    publisher: task.publisher.as_deref().and_then(text),
                    command: target.clone().unwrap_or_else(|| root.clone()),
                    target,
                    needs_admin: false,
                    removal: None,
                    switch: Switch::Package { key },
                    name,
                    location: "Packaged app".into(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
         xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
         xmlns:uap5="http://schemas.microsoft.com/appx/manifest/uap/windows10/5"
         xmlns:desktop="http://schemas.microsoft.com/appx/manifest/desktop/windows10">
  <Identity Name="OpenAI.ChatGPT-Desktop" Publisher="CN=OpenAI" Version="1.2.3.0"/>
  <Properties>
    <DisplayName>ChatGPT</DisplayName>
    <PublisherDisplayName>OpenAI</PublisherDisplayName>
  </Properties>
  <Applications>
    <Application Id="App" Executable="app\ChatGPT.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="ms-resource:AppName" Description="x"/>
      <Extensions>
        <uap5:Extension Category="windows.startupTask">
          <uap5:StartupTask TaskId="ChatGPTStartup" Enabled="true" DisplayName="ChatGPT"/>
        </uap5:Extension>
        <desktop:Extension Category="windows.startupTask" Executable="helper\tray.exe">
          <desktop:StartupTask TaskId="Tray" Enabled="false"/>
        </desktop:Extension>
        <uap:Extension Category="windows.protocol"><uap:Protocol Name="chatgpt"/></uap:Extension>
      </Extensions>
    </Application>
  </Applications>
</Package>"#;

    #[test]
    fn reads_package_startup_tasks() {
        let tasks = package_tasks(MANIFEST);
        assert_eq!(tasks.len(), 2);
        assert_eq!(
            tasks[0],
            PackageTask {
                id: "ChatGPTStartup".into(),
                enabled: true,
                names: vec!["ChatGPT".into(), "ms-resource:AppName".into(), "ChatGPT".into()],
                publisher: Some("OpenAI".into()),
                executable: Some(r"app\ChatGPT.exe".into()),
            }
        );
        assert_eq!(tasks[1].id, "Tray");
        assert!(!tasks[1].enabled);
        assert_eq!(tasks[1].names[0], "ms-resource:AppName");
        assert_eq!(tasks[1].executable.as_deref(), Some(r"helper\tray.exe"));
    }

    #[test]
    fn resource_addresses() {
        let uri = |raw| resource_uri(raw, "Microsoft.WindowsTerminal");
        assert_eq!(
            uri("ms-resource:AppName").as_deref(),
            Some("ms-resource://Microsoft.WindowsTerminal/Resources/AppName")
        );
        assert_eq!(
            uri("ms-resource:Strings/AppName").as_deref(),
            Some("ms-resource://Microsoft.WindowsTerminal/Strings/AppName")
        );
        assert_eq!(
            uri("ms-resource:///Resources/AppName").as_deref(),
            Some("ms-resource://Microsoft.WindowsTerminal/Resources/AppName")
        );
        assert_eq!(
            uri("ms-resource://Other/Resources/AppName").as_deref(),
            Some("ms-resource://Other/Resources/AppName")
        );
        assert_eq!(uri("Terminal"), None);
    }

    #[test]
    fn names_like_task_manager() {
        let desc = |d: &str| Some(d.to_string());
        assert_eq!(
            display_name(
                "MicrosoftEdgeAutoLaunch_1",
                Some(r"C:\Edge\msedge.exe"),
                desc("Microsoft Edge")
            ),
            "Microsoft Edge"
        );
        assert_eq!(
            display_name("Discord", Some(r"C:\Discord\Update.exe"), desc("Update")),
            "Discord"
        );
        assert_eq!(
            display_name(
                "Helper",
                Some(r"C:\Windows\System32\rundll32.exe"),
                desc("Windows host process (Rundll32)")
            ),
            "Helper"
        );
        assert_eq!(display_name("Tool", None, None), "Tool");
    }
}
