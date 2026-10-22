//! User settings, stored as plain TOML next to the app.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::Result;
use crate::{paths, AppState};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Which defaults the file was written with, so older ones can be updated.
    pub version: u32,
    pub storage: Storage,
    pub safety: Safety,
    pub scan: Scan,
    pub appearance: Appearance,
}

/// Where UWiper keeps its data. With `one_folder` on, everything lives in
/// the app folder and the custom paths are ignored.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Storage {
    pub one_folder: bool,
    pub quarantine: Option<String>,
    pub snapshots: Option<String>,
    pub logs: Option<String>,
    pub temp: Option<String>,
    pub log_days: u32,
}

impl Default for Storage {
    fn default() -> Self {
        Self {
            one_folder: true,
            quarantine: None,
            snapshots: None,
            logs: None,
            temp: None,
            log_days: 30,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Safety {
    pub quarantine: bool,
    pub keep_days: u32,
    pub restore_point: bool,
    pub protect_system: bool,
    /// "high", "high-medium" or "all"
    pub preselect: String,
}

impl Default for Safety {
    fn default() -> Self {
        Self {
            quarantine: true,
            keep_days: 30,
            restore_point: true,
            protect_system: true,
            // Medium finds are matched by name: worth a look, not a tick.
            preselect: "high".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Scan {
    /// "quick", "standard" or "deep"
    pub depth: String,
    pub look_in: LookIn,
    pub exclusions: Vec<String>,
}

impl Default for Scan {
    fn default() -> Self {
        Self {
            depth: "standard".into(),
            look_in: LookIn::default(),
            exclusions: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct LookIn {
    pub files: bool,
    pub registry: bool,
    pub services: bool,
    pub tasks: bool,
    pub drivers: bool,
    pub firewall: bool,
    pub shortcuts: bool,
    pub path: bool,
}

impl Default for LookIn {
    fn default() -> Self {
        Self {
            files: true,
            registry: true,
            services: true,
            tasks: true,
            drivers: true,
            firewall: true,
            shortcuts: true,
            path: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Appearance {
    pub accent: String,
    /// "windows" or "macos"
    pub window_buttons: String,
    pub translucent: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            accent: "#0071E3".into(),
            window_buttons: "windows".into(),
            translucent: true,
        }
    }
}

/// The current settings version.
const VERSION: u32 = 1;

/// Brings settings written by an older UWiper up to date.
fn migrate(mut s: Settings) -> (Settings, bool) {
    if s.version >= VERSION {
        return (s, false);
    }
    // 1.0: only High finds are picked to start with. "High and Medium" was
    // the old default, so it's taken as not chosen on purpose.
    if s.safety.preselect == "high-medium" {
        s.safety.preselect = "high".into();
    }
    s.version = VERSION;
    (s, true)
}

/// Reads settings.toml, writing the defaults on first launch so the file is
/// there to inspect from the start.
pub fn load() -> Settings {
    match std::fs::read_to_string(paths::settings_file()) {
        Ok(text) => {
            let (settings, changed) = migrate(toml::from_str(&text).unwrap_or_default());
            if changed {
                let _ = persist(&settings);
            }
            settings
        }
        Err(_) => {
            let defaults = Settings {
                version: VERSION,
                ..Settings::default()
            };
            let _ = persist(&defaults);
            defaults
        }
    }
}

fn persist(settings: &Settings) -> Result<()> {
    let file = paths::settings_file();
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Write-then-rename so a crash never leaves a half-written file.
    let tmp = file.with_extension("toml.tmp");
    std::fs::write(&tmp, toml::to_string_pretty(settings)?)?;
    std::fs::rename(&tmp, &file)?;
    Ok(())
}

// `async` keeps these off the main thread so the window never stalls.
#[tauri::command(async)]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command(async)]
pub fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<Settings> {
    persist(&settings)?;
    *state.settings.lock().unwrap() = settings.clone();
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: Settings = toml::from_str("[appearance]\naccent = \"#8E44EC\"\n").unwrap();
        assert_eq!(s.appearance.accent, "#8E44EC");
        assert_eq!(s.appearance.window_buttons, "windows");
        assert!(s.storage.one_folder);
        assert_eq!(s.safety.keep_days, 30);
    }

    #[test]
    fn older_settings_pick_only_high_finds() {
        let old: Settings = toml::from_str("[safety]\npreselect = \"high-medium\"\n").unwrap();
        let (new, changed) = migrate(old);
        assert!(changed && new.safety.preselect == "high" && new.version == VERSION);
        let chosen: Settings = toml::from_str("version = 1\n[safety]\npreselect = \"high-medium\"\n").unwrap();
        assert_eq!(
            migrate(chosen).0.safety.preselect,
            "high-medium",
            "a choice made since is kept"
        );
    }

    #[test]
    fn round_trips_through_toml() {
        let mut s = Settings::default();
        s.storage.quarantine = Some(r"E:\Quarantine".into());
        s.scan.exclusions.push(r"D:\Games".into());
        let text = toml::to_string_pretty(&s).unwrap();
        let back: Settings = toml::from_str(&text).unwrap();
        assert_eq!(back.storage.quarantine.as_deref(), Some(r"E:\Quarantine"));
        assert_eq!(back.scan.exclusions, vec![r"D:\Games".to_string()]);
    }
}
