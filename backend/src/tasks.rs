//! Scheduled tasks, read straight from the task XML files under
//! %SystemRoot%\System32\Tasks (needs admin rights).

use std::path::Path;

use serde::Serialize;
use tauri::State;

use crate::error::{blocking, Error, Result};
use crate::quarantine::settings_of;
use crate::targets::Target;
use crate::util::{decode_text, expand_env, program_state, resolve_executable, uses_per_user_vars};
use crate::{log, AppState};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TaskItem {
    pub(crate) name: String,
    folder: String,
    command: Option<String>,
    arguments: Option<String>,
    pub(crate) binary: Option<String>,
    pub(crate) file_exists: Option<bool>,
    enabled: bool,
    triggers: String,
    pub(crate) is_windows: bool,
    pub(crate) removal: Target,
}

#[tauri::command]
pub async fn list_tasks() -> Result<Vec<TaskItem>> {
    blocking(list).await
}

#[tauri::command]
pub async fn set_task_enabled(state: State<'_, AppState>, name: String, enabled: bool) -> Result<()> {
    let settings = settings_of(&state);
    blocking(move || {
        let task = list()?
            .into_iter()
            .find(|t| t.name == name)
            .ok_or(Error::Other("This task is gone – scan again".into()))?;
        if task.is_windows {
            return Err(Error::Other("Part of Windows – UWiper leaves it as is".into()));
        }
        let path = format!(r"\{}", task.name);
        crate::util::run_tool(
            "schtasks.exe",
            &["/Change", "/TN", &path, if enabled { "/ENABLE" } else { "/DISABLE" }],
        )?;
        let state = if enabled { "turned on" } else { "turned off" };
        log::write(&settings, &[format!("Scheduled task {path} {state}")]);
        Ok(())
    })
    .await
}

pub fn list() -> Result<Vec<TaskItem>> {
    if !cfg!(windows) {
        return Err(Error::Unsupported);
    }
    let root = std::path::PathBuf::from(expand_env(r"%SystemRoot%\System32\Tasks"));
    let mut out = Vec::new();
    let mut denied = false;
    for entry in walkdir::WalkDir::new(&root) {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                denied |= e
                    .io_error()
                    .is_some_and(|io| io.kind() == std::io::ErrorKind::PermissionDenied);
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(&root) else {
            continue;
        };
        match std::fs::read(entry.path()) {
            Ok(bytes) => {
                if let Some(task) = parse(&bytes, rel) {
                    out.push(task);
                }
            }
            Err(e) => denied |= e.kind() == std::io::ErrorKind::PermissionDenied,
        }
    }
    if out.is_empty() && denied {
        return Err(Error::NeedsAdmin);
    }
    out.sort_by_key(|t| t.name.to_lowercase());
    Ok(out)
}

fn trigger_label(tag: &str) -> &'static str {
    match tag {
        "LogonTrigger" => "At log on",
        "BootTrigger" => "At startup",
        "CalendarTrigger" => "On a schedule",
        "TimeTrigger" => "Once",
        "IdleTrigger" => "When idle",
        "EventTrigger" => "On an event",
        "RegistrationTrigger" => "When created",
        "SessionStateChangeTrigger" => "On session change",
        _ => "On a system event",
    }
}

fn parse(bytes: &[u8], rel: &Path) -> Option<TaskItem> {
    let text = decode_text(bytes);
    let doc = roxmltree::Document::parse(text.trim_start_matches('\u{feff}')).ok()?;
    let is = |n: &roxmltree::Node, tag: &str| n.is_element() && n.tag_name().name() == tag;
    let child_text = |parent: Option<roxmltree::Node>, tag: &str| {
        parent?
            .children()
            .find(|c| is(c, tag))
            .and_then(|c| c.text())
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
    };

    let exec = doc.descendants().find(|n| is(n, "Exec"));
    let settings = doc.descendants().find(|n| is(n, "Settings"));
    let enabled = child_text(settings, "Enabled").is_none_or(|v| v != "false");
    let command = child_text(exec, "Command");
    let arguments = child_text(exec, "Arguments");

    let mut triggers: Vec<&str> = Vec::new();
    if let Some(list) = doc.descendants().find(|n| is(n, "Triggers")) {
        for t in list.children().filter(|c| c.is_element()) {
            let label = trigger_label(t.tag_name().name());
            if !triggers.contains(&label) {
                triggers.push(label);
            }
        }
    }

    let name = rel.to_string_lossy().replace('/', "\\");
    let folder = rel
        .parent()
        .map(|p| p.to_string_lossy().replace('/', "\\"))
        .unwrap_or_default();
    let binary = command.as_deref().and_then(resolve_executable);
    Some(TaskItem {
        is_windows: name.starts_with("Microsoft\\"),
        // A task may run as another user, whose profile a per-user path points into.
        file_exists: if command.as_deref().is_some_and(uses_per_user_vars) {
            None
        } else {
            binary.as_deref().and_then(program_state)
        },
        binary,
        command,
        arguments,
        enabled,
        triggers: if triggers.is_empty() {
            "On demand".into()
        } else {
            triggers.join(", ")
        },
        folder,
        removal: Target::Task {
            path: format!(r"\{name}"),
        },
        name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TASK: &str = r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers><LogonTrigger><Enabled>true</Enabled></LogonTrigger><CalendarTrigger/></Triggers>
  <Settings><Enabled>false</Enabled></Settings>
  <Actions Context="Author"><Exec><Command>"C:\Gone\updater.exe"</Command><Arguments>/silent</Arguments></Exec></Actions>
</Task>"#;

    fn utf16le_with_bom(s: &str) -> Vec<u8> {
        let mut out = vec![0xFF, 0xFE];
        for unit in s.encode_utf16() {
            out.extend_from_slice(&unit.to_le_bytes());
        }
        out
    }

    #[test]
    fn reads_utf16_task_files() {
        let task = parse(&utf16le_with_bom(TASK), Path::new("Vendor/Updater")).unwrap();
        assert_eq!(task.name, r"Vendor\Updater");
        assert_eq!(task.folder, "Vendor");
        assert_eq!(task.command.as_deref(), Some(r#""C:\Gone\updater.exe""#));
        assert_eq!(task.arguments.as_deref(), Some("/silent"));
        assert_eq!(task.binary.as_deref(), Some(r"C:\Gone\updater.exe"));
        assert!(!task.enabled);
        assert_eq!(task.triggers, "At log on, On a schedule");
        assert!(!task.is_windows);
    }
}
