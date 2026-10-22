//! Scheduled tasks, read straight from the task XML files under
//! %SystemRoot%\System32\Tasks (needs admin rights).

use std::path::Path;

use serde::Serialize;
use tauri::State;

use crate::error::{blocking, Error, Result};
use crate::quarantine::settings_of;
use crate::targets::Target;
use crate::util::{
    command_executable, decode_text, expand_env, program_state, resolve_executable, uses_per_user_vars, volume_root,
};
use crate::{log, AppState};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TaskItem {
    pub(crate) name: String,
    folder: String,
    command: Option<String>,
    arguments: Option<String>,
    /// The program it runs: the first one that's there, of all its actions.
    pub(crate) binary: Option<String>,
    /// Every program its actions run, where UWiper can tell.
    #[serde(skip)]
    pub(crate) programs: Vec<String>,
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

/// "Microsoft Corporation", or a resource string from a Windows file such as
/// `$(@%SystemRoot%\system32\wininet.dll,-16000)`.
fn is_windows_author(author: &str) -> bool {
    let a = author.trim().to_lowercase();
    a.starts_with("microsoft") || a.starts_with("$(@%systemroot%") || a.starts_with("$(@%windir%")
}

/// Where an action's program is, the way Task Scheduler finds it: a full
/// path as it is, a relative one in its working directory, a bare name
/// there or in Windows' folders and on PATH. `None` when that can't be told
/// – a relative path without a full working directory.
fn task_program(command: &str, dir: Option<&str>) -> Option<String> {
    let exe = command_executable(command)?;
    if volume_root(&exe).is_some() {
        return Some(exe);
    }
    let dir = dir
        .map(|d| expand_env(d.trim().trim_matches('"')))
        .filter(|d| volume_root(d).is_some());
    let in_dir = dir.map(|d| format!(r"{}\{}", d.trim_end_matches('\\'), exe.trim_start_matches(".\\")));
    if exe.contains(['\\', '/']) {
        return in_dir;
    }
    in_dir
        .clone()
        .filter(|p| Path::new(p).is_file())
        .or_else(|| resolve_executable(&exe))
        .or(in_dir)
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

    // A task can have up to 32 actions; COM handlers and the like run no file UWiper can check.
    let actions: Vec<roxmltree::Node> = doc
        .descendants()
        .find(|n| is(n, "Actions"))
        .map(|a| a.children().filter(|c| c.is_element()).collect())
        .unwrap_or_default();
    let execs: Vec<roxmltree::Node> = actions.iter().copied().filter(|a| is(a, "Exec")).collect();
    let other_actions = actions.len() > execs.len();
    let exec = execs.first().copied();
    let author = child_text(doc.descendants().find(|n| is(n, "RegistrationInfo")), "Author").unwrap_or_default();
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
    // A task may run as another user, whose profile a per-user path points into.
    let programs: Vec<Option<String>> = execs
        .iter()
        .map(|e| {
            let command = child_text(Some(*e), "Command")?;
            if uses_per_user_vars(&command) {
                return None;
            }
            task_program(&command, child_text(Some(*e), "WorkingDirectory").as_deref())
        })
        .collect();
    let states: Vec<Option<bool>> = programs.iter().map(|p| p.as_deref().and_then(program_state)).collect();
    // Missing only when every action runs a program that's surely gone.
    let file_exists = if states.contains(&Some(true)) {
        Some(true)
    } else if !other_actions && !states.is_empty() && states.iter().all(|s| *s == Some(false)) {
        Some(false)
    } else {
        None
    };
    let binary = programs
        .iter()
        .zip(&states)
        .find(|(_, s)| **s == Some(true))
        .or_else(|| programs.iter().zip(&states).find(|(p, _)| p.is_some()))
        .and_then(|(p, _)| p.clone());
    let programs: Vec<String> = programs.into_iter().flatten().collect();
    Some(TaskItem {
        // Windows' own tasks also sit outside the Microsoft folder; their author says whose they are.
        is_windows: name.starts_with("Microsoft\\") || is_windows_author(&author),
        file_exists,
        binary,
        programs,
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

    #[test]
    fn finds_programs_the_way_task_scheduler_does() {
        assert_eq!(
            task_program(r#""C:\Tools\App\app.exe" --wake"#, None).as_deref(),
            Some(r"C:\Tools\App\app.exe")
        );
        assert_eq!(
            task_program(r"bin\app.exe", Some(r"C:\Tools\App\")).as_deref(),
            Some(r"C:\Tools\App\bin\app.exe")
        );
        assert_eq!(task_program(r"bin\app.exe", None), None, "relative to what?");
        assert_eq!(task_program(r"bin\app.exe", Some(r"..\x")), None);
    }

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

    #[test]
    fn windows_tasks_outside_the_microsoft_folder() {
        let xml = TASK.replace(
            "<Triggers>",
            "<RegistrationInfo><Author>$(@%SystemRoot%\\system32\\shell32.dll,-1)</Author></RegistrationInfo><Triggers>",
        );
        let task = parse(xml.as_bytes(), Path::new("CreateExplorerShellUnelevatedTask")).unwrap();
        assert!(task.is_windows);
        assert!(is_windows_author("Microsoft Corporation"));
        assert!(!is_windows_author("Google LLC"));
    }

    #[test]
    fn a_task_is_broken_only_when_every_action_is() {
        // A second action whose program is there (this test binary).
        let here = std::env::current_exe().unwrap().to_string_lossy().into_owned();
        let two = TASK.replace(
            "</Exec></Actions>",
            &format!("</Exec><Exec><Command>{here}</Command></Exec></Actions>"),
        );
        let task = parse(two.as_bytes(), Path::new("Vendor/Two")).unwrap();
        assert_ne!(task.file_exists, Some(false));
        let com = TASK.replace(
            "</Exec></Actions>",
            "</Exec><ComHandler><ClassId>{00000000-0000-0000-0000-000000000000}</ClassId></ComHandler></Actions>",
        );
        let task = parse(com.as_bytes(), Path::new("Vendor/Com")).unwrap();
        assert_eq!(task.file_exists, None);
    }
}
