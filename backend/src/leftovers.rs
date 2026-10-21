//! Finds what an app left behind after its uninstaller ran: its folder,
//! data folders and registry keys named after it, shortcuts, services, tasks
//! and startup entries that point into its folder. Every find has a
//! confidence level, and nothing that belongs to Windows, to another
//! installed app or to the "Never touch" list is ever offered.
#![cfg_attr(not(windows), allow(dead_code))]

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{blocking, Result};
use crate::settings::Settings;
use crate::targets::{self, Guard, Target};
use crate::util::{dir_size, expand_env, is_within, norm_path, parallel_map};
use crate::{apps, AppState};

/// The app as the UI knew it before uninstalling.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppRef {
    pub id: String,
    pub name: String,
    pub publisher: Option<String>,
    pub install_location: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Leftover {
    id: String,
    /// "folder", "file", "shortcut", "registry", "service", "task" or "startup"
    kind: &'static str,
    label: String,
    reason: String,
    /// "high", "medium" or "low"
    confidence: &'static str,
    size_bytes: Option<u64>,
    needs_admin: bool,
    target: Target,
}

#[tauri::command]
pub async fn scan_leftovers(state: State<'_, AppState>, app: AppRef) -> Result<Vec<Leftover>> {
    let settings = state.settings.lock().unwrap().clone();
    blocking(move || scan(&settings, &app)).await
}

// ---------------------------------------------------------------------------
// Names

/// Words in app names that aren't part of the name itself.
fn is_noise(word: &str) -> bool {
    let w = word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    // Versions such as 25.01, v4.2 or 2015-2022.
    let version = w
        .trim_start_matches('v')
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
        && w.chars().any(|c| c.is_ascii_digit());
    w.is_empty() || version || matches!(w.as_str(), "x64" | "x86" | "64-bit" | "32-bit" | "version" | "edition")
}

/// "7-Zip 25.01 (x64)" → "7-Zip", "Python 3.12.4 (64-bit)" → "Python"
pub fn clean_name(name: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for ch in name.chars() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    let words: Vec<&str> = out.split_whitespace().filter(|w| !is_noise(w)).collect();
    words.join(" ").trim_end_matches([' ', '-', '–', ',']).to_string()
}

/// Lowercase letters and digits only, for comparing names with folder names.
pub fn squash(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// "Discord Inc." → "discord", "Valve Corporation" → "valve"
pub fn clean_publisher(publisher: &str) -> String {
    const SUFFIXES: [&str; 14] = [
        "inc",
        "llc",
        "ltd",
        "limited",
        "corp",
        "corporation",
        "co",
        "company",
        "gmbh",
        "ag",
        "sa",
        "srl",
        "bv",
        "the",
    ];
    let words: Vec<&str> = publisher
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|w| !w.is_empty() && !SUFFIXES.contains(&squash(w).as_str()))
        .collect();
    squash(&words.join(" "))
}

/// Folder and key names that are too general to tie to one app.
const GENERIC: [&str; 34] = [
    "app",
    "apps",
    "application",
    "bin",
    "cache",
    "caches",
    "common",
    "config",
    "data",
    "default",
    "desktop",
    "driver",
    "drivers",
    "files",
    "local",
    "logs",
    "microsoft",
    "packages",
    "program",
    "programs",
    "roaming",
    "runtime",
    "service",
    "services",
    "settings",
    "shared",
    "software",
    "system",
    "temp",
    "tmp",
    "tools",
    "update",
    "updater",
    "windows",
];

#[derive(Debug, Default)]
struct Names {
    /// Squashed forms of the app's name that a folder or key may carry.
    app: Vec<String>,
    /// The squashed publisher, when it's specific enough to match on.
    publisher: Option<String>,
}

impl Names {
    fn new(app: &AppRef, taken: &HashSet<String>) -> Self {
        let clean = clean_name(&app.name);
        let mut forms = vec![squash(&clean)];
        // "Mozilla Firefox" is often just "Firefox" on disk.
        let publisher = app.publisher.as_deref().map(clean_publisher).unwrap_or_default();
        if let Some((first, rest)) = clean.split_once(' ') {
            if !publisher.is_empty() && squash(first) == publisher {
                forms.push(squash(rest));
            }
        }
        if let Some(folder) = app
            .install_location
            .as_deref()
            .and_then(|p| p.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next())
        {
            if squash(&clean_name(folder)).len() >= 3 {
                forms.push(squash(&clean_name(folder)));
            }
        }
        forms.sort();
        forms.dedup();
        // Another installed app with the same name would share the folder.
        forms.retain(|f| f.len() >= 3 && !GENERIC.contains(&f.as_str()) && !taken.contains(f));
        let publisher = (publisher.len() >= 3 && !GENERIC.contains(&publisher.as_str())).then_some(publisher);
        Names { app: forms, publisher }
    }

    fn is_app(&self, name: &str) -> bool {
        let s = squash(name);
        self.app.contains(&s) || self.app.contains(&squash(&clean_name(name)))
    }

    fn is_publisher(&self, name: &str) -> bool {
        self.publisher.as_deref().is_some_and(|p| clean_publisher(name) == p)
    }
}

// ---------------------------------------------------------------------------
// Collecting

struct Found {
    guard: Guard,
    /// Other installed apps' folders: never removed, nor anything holding them.
    other_folders: Vec<String>,
    items: Vec<Leftover>,
    seen: HashSet<String>,
}

impl Found {
    fn add(&mut self, kind: &'static str, target: Target, reason: &str, confidence: &'static str) {
        let label = targets::describe(&target);
        let id = norm_path(&label);
        if self.seen.contains(&id) || self.guard.check(&target).is_err() {
            return;
        }
        if let Target::Path { path } = &target {
            if self.other_folders.iter().any(|other| is_within(other, path)) {
                return;
            }
        }
        self.seen.insert(id.clone());
        let needs_admin = targets::needs_admin(&target);
        self.items.push(Leftover {
            id,
            kind,
            label,
            reason: reason.into(),
            confidence,
            size_bytes: None,
            needs_admin,
            target,
        });
    }

    fn add_path(&mut self, path: &Path, reason: &str, confidence: &'static str) {
        let kind = if path.is_dir() {
            "folder"
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("lnk") || e.eq_ignore_ascii_case("url"))
        {
            "shortcut"
        } else {
            "file"
        };
        self.add(
            kind,
            Target::Path {
                path: path.to_string_lossy().into_owned(),
            },
            reason,
            confidence,
        );
    }

    /// Drops finds that sit inside a folder that is already on the list,
    /// measures folders and puts the surest finds first.
    fn finish(mut self) -> Vec<Leftover> {
        let folders: Vec<String> = self
            .items
            .iter()
            .filter(|i| i.kind == "folder")
            .map(|i| i.label.clone())
            .collect();
        self.items.retain(|item| match &item.target {
            Target::Path { path } => !folders.iter().any(|f| f != path && is_within(path, f)),
            _ => true,
        });
        let sizes = parallel_map(&self.items, |item| match &item.target {
            Target::Path { path } => Some(match std::fs::metadata(path) {
                Ok(meta) if meta.is_dir() => dir_size(Path::new(path)).0,
                Ok(meta) => meta.len(),
                Err(_) => 0,
            }),
            _ => None,
        });
        for (item, size) in self.items.iter_mut().zip(sizes) {
            item.size_bytes = size;
        }
        let rank = |c: &str| match c {
            "high" => 0,
            "medium" => 1,
            _ => 2,
        };
        self.items
            .sort_by_key(|i| (rank(i.confidence), i.kind, i.label.to_lowercase()));
        self.items
    }
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|it| it.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// True when a shortcut file mentions the folder, which is where its target
/// path is stored. Shortcuts keep paths as ANSI or UTF-16 text.
pub fn shortcut_points_into(bytes: &[u8], folder: &str) -> bool {
    let needle = norm_path(folder);
    if needle.len() < 4 {
        return false;
    }
    let utf16 = |skip: usize| {
        let units: Vec<u16> = bytes[skip..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units).to_lowercase()
    };
    String::from_utf8_lossy(bytes).to_lowercase().contains(&needle)
        || utf16(0).contains(&needle)
        || (bytes.len() > 1 && utf16(1).contains(&needle))
}

/// Folders where apps keep their data, with how sure a name match there is.
/// Documents often holds the user's own files (saves, projects), so a match
/// there is only a suggestion.
fn data_roots(deep: bool) -> Vec<(PathBuf, &'static str)> {
    let mut roots = vec![
        ("%APPDATA%", "medium"),
        ("%LOCALAPPDATA%", "medium"),
        (r"%USERPROFILE%\AppData\LocalLow", "medium"),
        (r"%LOCALAPPDATA%\Programs", "medium"),
        ("%ProgramData%", "medium"),
    ];
    if deep {
        roots.extend([
            ("%ProgramFiles%", "medium"),
            ("%ProgramFiles(x86)%", "medium"),
            (r"%USERPROFILE%\Documents", "low"),
        ]);
    }
    roots
        .into_iter()
        .map(|(p, confidence)| (expand_env(p), confidence))
        .filter(|(p, _)| !p.contains('%'))
        .map(|(p, confidence)| (PathBuf::from(p), confidence))
        .collect()
}

fn shortcut_dirs() -> Vec<(PathBuf, usize)> {
    [
        (r"%APPDATA%\Microsoft\Windows\Start Menu\Programs", 2),
        (r"%ProgramData%\Microsoft\Windows\Start Menu\Programs", 2),
        (r"%USERPROFILE%\Desktop", 1),
        (r"%PUBLIC%\Desktop", 1),
    ]
    .into_iter()
    .map(|(p, depth)| (PathBuf::from(expand_env(p)), depth))
    .filter(|(p, _)| !p.to_string_lossy().contains('%'))
    .collect()
}

// ---------------------------------------------------------------------------
// Scanning

pub fn scan(settings: &Settings, app: &AppRef) -> Result<Vec<Leftover>> {
    let others: Vec<apps::InstalledApp> = apps::list_plain()?.into_iter().filter(|a| a.id != app.id).collect();
    let taken: HashSet<String> = others.iter().map(|a| squash(&clean_name(&a.name))).collect();
    let publisher_shared = app.publisher.as_deref().map(clean_publisher).is_some_and(|p| {
        others
            .iter()
            .any(|o| o.publisher.as_deref().map(clean_publisher).as_deref() == Some(p.as_str()))
    });
    let names = Names::new(app, &taken);
    let install = app
        .install_location
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| p.trim_end_matches('\\').to_string());
    let inside_install = |path: &str| install.as_deref().is_some_and(|dir| is_within(path, dir));

    let mut found = Found {
        guard: Guard::new(settings),
        other_folders: others
            .iter()
            .filter_map(|o| o.install_location.as_deref())
            .map(norm_path)
            .collect(),
        items: Vec::new(),
        seen: HashSet::new(),
    };
    let look = &settings.scan.look_in;
    let quick = settings.scan.depth == "quick";
    let deep = settings.scan.depth == "deep";

    if look.files {
        if let Some(dir) = install.as_deref().filter(|d| Path::new(d).exists()) {
            found.add_path(
                Path::new(dir),
                "The app's folder, still there after uninstalling",
                "high",
            );
        }
        for (root, confidence) in data_roots(deep) {
            for dir in subdirs(&root) {
                let name = file_name(&dir);
                if names.is_app(&name) {
                    found.add_path(&dir, "Named after the app", confidence);
                } else if !quick && names.is_publisher(&name) {
                    for sub in subdirs(&dir).iter().filter(|d| names.is_app(&file_name(d))) {
                        found.add_path(sub, "Named after the app, in its publisher's folder", confidence);
                    }
                    if deep && !publisher_shared {
                        found.add_path(
                            &dir,
                            "The publisher's folder – no other app from them is installed",
                            "low",
                        );
                    }
                }
            }
        }
    }

    if look.shortcuts {
        for (dir, depth) in shortcut_dirs() {
            for entry in walkdir::WalkDir::new(&dir)
                .min_depth(1)
                .max_depth(depth)
                .into_iter()
                .flatten()
            {
                let path = entry.path();
                let name = file_name(path);
                let stem = name.rsplit_once('.').map_or(name.as_str(), |(s, _)| s).to_string();
                if entry.file_type().is_dir() {
                    if names.is_app(&name) || (!publisher_shared && names.is_publisher(&name)) {
                        found.add_path(path, "Start menu folder for the app", "medium");
                    }
                    continue;
                }
                let is_link = path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("lnk") || e.eq_ignore_ascii_case("url"));
                if !is_link {
                    continue;
                }
                let points_in = install.as_deref().is_some_and(|dir| {
                    std::fs::read(path).is_ok_and(|bytes| bytes.len() < 1 << 20 && shortcut_points_into(&bytes, dir))
                });
                if points_in {
                    found.add_path(path, "Shortcut to the app's folder", "high");
                } else if names.is_app(&stem) {
                    found.add_path(path, "Shortcut named after the app", "medium");
                }
            }
        }
    }

    // Startup entries are files or registry values, so they follow either switch.
    if look.files || look.registry {
        for item in crate::startup::list().unwrap_or_default() {
            if item.target.as_deref().is_some_and(inside_install) {
                found.add("startup", item.removal, "Starts the removed app with Windows", "high");
            }
        }
    }

    if look.registry {
        if let Some(key) = apps::uninstall_key_path(&app.id).filter(|_| imp::key_exists(&app.id)) {
            found.add(
                "registry",
                Target::RegistryKey { key },
                "Uninstall entry left behind",
                "high",
            );
        }
        for software in [r"HKCU\Software", r"HKLM\SOFTWARE", r"HKLM\SOFTWARE\WOW6432Node"] {
            for name in imp::subkeys(software) {
                let key = format!(r"{software}\{name}");
                if names.is_app(&name) {
                    found.add(
                        "registry",
                        Target::RegistryKey { key },
                        "Settings named after the app",
                        "medium",
                    );
                } else if !quick && names.is_publisher(&name) {
                    for sub in imp::subkeys(&key).into_iter().filter(|s| names.is_app(s)) {
                        let key = format!(r"{key}\{sub}");
                        found.add(
                            "registry",
                            Target::RegistryKey { key },
                            "The app's settings under its publisher",
                            "medium",
                        );
                    }
                    if deep && !publisher_shared {
                        let reason = "The publisher's settings – no other app from them is installed";
                        found.add("registry", Target::RegistryKey { key }, reason, "low");
                    }
                }
            }
        }
    }

    if look.services {
        for service in crate::services::list().unwrap_or_default() {
            if service.binary.as_deref().is_some_and(inside_install) && !service.is_windows {
                let reason = format!(
                    "Service \"{}\" runs a program from the app's folder",
                    service.display_name
                );
                found.add("service", service.removal, &reason, "high");
            }
        }
    }

    if look.tasks {
        for task in crate::tasks::list().unwrap_or_default() {
            let last = task.name.rsplit('\\').next().unwrap_or_default().to_string();
            if task.binary.as_deref().is_some_and(inside_install) {
                found.add("task", task.removal, "Runs a program from the app's folder", "high");
            } else if !task.is_windows && (names.is_app(&last) || task.name.split('\\').any(|part| names.is_app(part)))
            {
                found.add("task", task.removal, "Named after the app", "medium");
            }
        }
    }

    Ok(found.finish())
}

#[cfg(not(windows))]
mod imp {
    pub fn key_exists(_id: &str) -> bool {
        false
    }

    pub fn subkeys(_key: &str) -> Vec<String> {
        Vec::new()
    }
}

#[cfg(windows)]
mod imp {
    use crate::targets::split_key;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY};
    use winreg::RegKey;

    fn open(key: &str) -> Option<RegKey> {
        let (hive, sub) = split_key(key)?;
        let root = RegKey::predef(if hive == "HKLM" {
            HKEY_LOCAL_MACHINE
        } else {
            HKEY_CURRENT_USER
        });
        root.open_subkey_with_flags(sub, KEY_READ | KEY_WOW64_64KEY).ok()
    }

    pub fn key_exists(id: &str) -> bool {
        crate::apps::uninstall_key_path(id).is_some_and(|key| open(&key).is_some())
    }

    pub fn subkeys(key: &str) -> Vec<String> {
        open(key).map(|k| k.enum_keys().flatten().collect()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_app_names() {
        assert_eq!(clean_name("7-Zip 25.01 (x64)"), "7-Zip");
        assert_eq!(clean_name("Python 3.12.4 (64-bit)"), "Python");
        assert_eq!(clean_name("Notepad++ (64-bit x64)"), "Notepad++");
        assert_eq!(clean_name("Mozilla Firefox (x64 en-US)"), "Mozilla Firefox");
        assert_eq!(clean_name("Telegram Desktop version 6.1.3"), "Telegram Desktop");
        assert_eq!(clean_name("Ravenswatch"), "Ravenswatch");
        assert_eq!(clean_name("Blender v4.2"), "Blender");
        assert_eq!(clean_name("3DMark"), "3DMark");
    }

    #[test]
    fn cleans_publishers() {
        assert_eq!(clean_publisher("Discord Inc."), "discord");
        assert_eq!(clean_publisher("Valve Corporation"), "valve");
        assert_eq!(clean_publisher("Mozilla"), "mozilla");
        assert_eq!(clean_publisher("Some Company, LLC"), "some");
    }

    fn app(name: &str, publisher: &str, location: Option<&str>) -> AppRef {
        AppRef {
            id: "x".into(),
            name: name.into(),
            publisher: Some(publisher.into()),
            install_location: location.map(Into::into),
        }
    }

    #[test]
    fn matches_folders_named_after_the_app() {
        let names = Names::new(
            &app("OBS Studio", "OBS Project", Some(r"C:\Program Files\obs-studio")),
            &HashSet::new(),
        );
        assert!(names.is_app("obs-studio"));
        assert!(names.is_app("OBS Studio"));
        assert!(!names.is_app("obs"));
        assert!(names.is_publisher("OBS Project"));

        let firefox = Names::new(&app("Mozilla Firefox (x64 en-US)", "Mozilla", None), &HashSet::new());
        assert!(firefox.is_app("Firefox"));
        assert!(firefox.is_app("Mozilla Firefox"));
        assert!(!firefox.is_app("Mozilla"));
    }

    #[test]
    fn never_matches_generic_or_shared_names() {
        let taken: HashSet<String> = ["python".to_string()].into();
        let python = Names::new(
            &app("Python 3.12.4 (64-bit)", "Python Software Foundation", None),
            &taken,
        );
        assert!(!python.is_app("Python"), "another Python is installed");
        let updater = Names::new(&app("Updater", "Microsoft Corporation", None), &HashSet::new());
        assert!(!updater.is_app("Updater"));
        assert!(!updater.is_publisher("Microsoft"));
    }

    #[test]
    fn reads_shortcut_targets() {
        let mut lnk = b"L\0\0\0junk C:\\Program Files\\Ravenswatch\\Ravenswatch.exe junk".to_vec();
        assert!(shortcut_points_into(&lnk, r"C:\Program Files\Ravenswatch"));
        assert!(!shortcut_points_into(&lnk, r"C:\Program Files\Other"));
        lnk = r"x C:\Games\Ravenswatch\Game.exe"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        assert!(shortcut_points_into(&lnk, r"c:\games\ravenswatch"));
    }
}
