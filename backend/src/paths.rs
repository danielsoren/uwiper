//! Where UWiper keeps things. By default everything lives in one folder: the
//! one the executable sits in. Nothing goes to AppData or ProgramData.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::State;

use crate::error::{blocking, Result};
use crate::settings::Settings;
use crate::util::{dir_size, expand_env, is_within};
use crate::AppState;

/// The UWiper folder. `UWIPER_HOME` overrides it; debug builds use a folder
/// under `target/` so development runs don't mix with real data.
pub fn home() -> PathBuf {
    if let Some(dir) = std::env::var_os("UWIPER_HOME") {
        return PathBuf::from(dir);
    }
    if cfg!(debug_assertions) {
        return Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("dev-home");
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn settings_file() -> PathBuf {
    home().join("settings.toml")
}

/// WebView2 keeps its profile here instead of in %LOCALAPPDATA%.
pub fn webview_dir() -> PathBuf {
    home().join("webview")
}

pub const DATA_DIRS: [&str; 4] = ["quarantine", "snapshots", "logs", "temp"];

pub fn data_dir(settings: &Settings, key: &str) -> PathBuf {
    let custom = match key {
        "quarantine" => &settings.storage.quarantine,
        "snapshots" => &settings.storage.snapshots,
        "logs" => &settings.storage.logs,
        _ => &settings.storage.temp,
    };
    match custom {
        Some(path) if !settings.storage.one_folder && !path.trim().is_empty() => PathBuf::from(expand_env(path.trim())),
        _ => home().join(key),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataDir {
    key: &'static str,
    path: String,
    inside: bool,
    exists: bool,
    size_bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    home: String,
    settings_file: String,
    home_size_bytes: u64,
    dirs: Vec<DataDir>,
}

fn is_inside(path: &Path, root: &Path) -> bool {
    is_within(&path.to_string_lossy(), &root.to_string_lossy())
}

/// Counts only what UWiper put in its folder, so keeping the exe in a big
/// folder like Downloads doesn't mean walking all of it.
fn own_size(home: &Path, dirs: &[DataDir]) -> u64 {
    let file_len = |path: PathBuf| std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let exe = std::env::current_exe()
        .ok()
        .filter(|exe| exe.parent() == Some(home))
        .map_or(0, file_len);
    let inside: u64 = dirs.iter().filter(|d| d.inside).map(|d| d.size_bytes).sum();
    exe + file_len(settings_file()) + dir_size(&webview_dir()).0 + inside
}

#[tauri::command]
pub async fn storage_info(state: State<'_, AppState>) -> Result<StorageInfo> {
    let settings = state.settings.lock().unwrap().clone();
    blocking(move || {
        let home = home();
        let dirs: Vec<DataDir> = DATA_DIRS
            .iter()
            .map(|&key| {
                let path = data_dir(&settings, key);
                DataDir {
                    key,
                    inside: is_inside(&path, &home),
                    exists: path.is_dir(),
                    size_bytes: dir_size(&path).0,
                    path: path.to_string_lossy().into_owned(),
                }
            })
            .collect();
        Ok(StorageInfo {
            home_size_bytes: own_size(&home, &dirs),
            home: home.to_string_lossy().into_owned(),
            settings_file: settings_file().to_string_lossy().into_owned(),
            dirs,
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_paths_inside_the_home_folder() {
        let home = Path::new(r"D:\Tools\UWiper");
        assert!(is_inside(Path::new(r"D:\Tools\UWiper\quarantine"), home));
        assert!(is_inside(Path::new(r"d:\tools\uwiper\logs"), home));
        assert!(!is_inside(Path::new(r"D:\Tools\UWiper-old\logs"), home));
        assert!(!is_inside(Path::new(r"E:\Quarantine"), home));
    }

    #[test]
    fn custom_paths_only_apply_when_one_folder_is_off() {
        let mut s = Settings::default();
        s.storage.quarantine = Some(r"E:\Q".into());
        assert_eq!(data_dir(&s, "quarantine"), home().join("quarantine"));
        s.storage.one_folder = false;
        assert_eq!(data_dir(&s, "quarantine"), PathBuf::from(r"E:\Q"));
        assert_eq!(data_dir(&s, "logs"), home().join("logs"));
    }
}
