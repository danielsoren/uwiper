//! Install tracking: a snapshot of the PC before an installer runs and another
//! after it. What's new is saved as the app's trace, and a later uninstall
//! uses it to find everything the installer added.
//!
//! Private: the snapshot on disk holds only salted hashes of names, and a
//! trace holds names – of files, folders, registry keys and the like – never
//! anything inside them. Both stay in UWiper's own folder.
//!
//! Accurate: each place is looked at two levels down – a new folder in Program
//! Files, or a new folder inside a vendor's folder that was already there.
//! Anything deeper inside an existing folder is left out, since other apps
//! running at the same time write there too. Something only counts as new when
//! it was also made after tracking started. A trace remembers what was inside
//! each new folder, so a folder that later holds other things too is never
//! removed whole.
#![cfg_attr(not(windows), allow(dead_code))]

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{blocking, Error, Result};
use crate::leftovers::{clean_name, clean_publisher, squash, Found, Leftover};
use crate::paths::data_dir;
use crate::quarantine::{settings_of, slug};
use crate::settings::Settings;
use crate::targets::{self, Guard, Target};
use crate::util::{canonical, dir_stats, is_within, norm_path, now_secs, parallel_map, utc_date};
use crate::{apps, log, AppState};

/// Longer than this, and other apps have had too much time to add things:
/// nothing is ticked for the user.
const LONG_WINDOW_SECS: u64 = 3 * 3600;

// ---------------------------------------------------------------------------
// What's saved

/// Something the installer added.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TraceItem {
    /// Stable and unique within a trace.
    id: String,
    /// "folder", "file", "shortcut", "registry", "app", "service", "task",
    /// "startup", "environment" or "driver"
    kind: String,
    label: String,
    target: Target,
    /// Surely the installer's: made at the top of its place, or a service,
    /// task or startup entry that runs a program it added.
    sure: bool,
    /// For a driver package: its Driver Store folder, which tells it apart
    /// from a later package Windows publishes under the same oem name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    store_folder: Option<String>,
    /// For a folder or registry key: what was in it when the trace was saved
    /// (lowercase names). Anything else in it later belongs to someone else.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    children: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Trace {
    id: String,
    name: String,
    /// The installed app it belongs to (`hive|view|key`, as in the app list).
    app_id: Option<String>,
    publisher: Option<String>,
    version: Option<String>,
    /// The installer's file name.
    installer: Option<String>,
    started_at: u64,
    finished_at: u64,
    /// Other apps that were installed along with it; their things aren't part of the trace.
    also_installed: Vec<String>,
    items: Vec<TraceItem>,
}

/// About the tracking under way: small, read often.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct Info {
    started_at: u64,
    /// The Windows user it was started for: another user's AppData would all look new.
    user: String,
    /// Started with administrator rights: without them, less can be read.
    elevated: bool,
    installer: Option<String>,
    /// Mixed into every hash of the snapshot.
    salt: String,
}

/// The "before" snapshot: salted hashes only, for telling what's new.
#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    keys: BTreeSet<String>,
    /// The places that were looked at.
    roots: BTreeSet<String>,
    /// Installed apps (id → version).
    apps: BTreeMap<String, String>,
    /// Installed apps by clean name and publisher, to tell an upgrade under a new id.
    app_names: BTreeSet<String>,
}

/// Something to choose as the app a trace belongs to.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    id: String,
    name: String,
}

/// What `finish_tracking` found, for the user to look over before saving.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    started_at: u64,
    name: String,
    app_id: Option<String>,
    publisher: Option<String>,
    version: Option<String>,
    /// The app was already installed and this was an update.
    updated: bool,
    also_installed: Vec<String>,
    /// Apps that were installed or changed meanwhile, to pick from.
    candidates: Vec<Candidate>,
    /// False when scheduled tasks couldn't be read (without administrator rights).
    tasks_checked: bool,
    /// Tracking ran so long that nothing is ticked.
    long_window: bool,
    items: Vec<DraftItem>,
    /// Its entry in installed apps, added on save when it's new.
    #[serde(skip)]
    app_item: Option<TraceItem>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DraftItem {
    #[serde(flatten)]
    item: TraceItem,
    size_bytes: Option<u64>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TrackingStatus {
    active: bool,
    started_at: Option<u64>,
    /// Tracking was started with administrator rights.
    elevated: bool,
    installer: Option<String>,
    /// UWiper started the installer and can see whether it's running.
    installer_followed: bool,
    installer_running: bool,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TraceView {
    #[serde(flatten)]
    trace: Trace,
    /// Its app (or an update of it) is still installed.
    installed: bool,
    /// For each item: it's still there.
    present: Vec<bool>,
}

// ---------------------------------------------------------------------------
// Looking at the PC

/// One thing seen on the PC.
#[derive(Clone, Debug)]
struct Entry {
    /// Lowercase, with a prefix per kind of thing: what snapshots compare.
    key: String,
    /// The key of the folder or registry key it's in, when that was looked at too.
    parent: Option<String>,
    kind: &'static str,
    target: Target,
    label: Option<String>,
    sure: bool,
    /// The program a service, task or startup entry runs.
    program: Option<String>,
    store_folder: Option<String>,
}

impl Entry {
    fn new(key: String, parent: Option<String>, kind: &'static str, target: Target, sure: bool) -> Self {
        Entry {
            key,
            parent,
            kind,
            target,
            label: None,
            sure,
            program: None,
            store_folder: None,
        }
    }
}

#[derive(Clone, Debug)]
struct AppInfo {
    id: String,
    name: String,
    publisher: Option<String>,
    version: Option<String>,
    /// Its own folder: reported, or where its uninstaller is.
    location: Option<String>,
}

impl AppInfo {
    /// Clean name and publisher, which stay the same across upgrades.
    fn identity(&self) -> String {
        identity(&self.name, self.publisher.as_deref())
    }
}

fn identity(name: &str, publisher: Option<&str>) -> String {
    format!(
        "{}|{}",
        squash(&clean_name(name)),
        squash(&clean_publisher(publisher.unwrap_or_default()))
    )
}

#[derive(Clone, Debug, Default)]
struct Look {
    entries: Vec<Entry>,
    /// Names seen that aren't entries themselves (service keys that couldn't be opened).
    seen: BTreeSet<String>,
    /// Places that couldn't be read (`x:` keys). Nothing in them counts as new
    /// later, since it may have been there all along.
    unreadable: BTreeSet<String>,
    roots: BTreeSet<String>,
    apps: Vec<AppInfo>,
}

/// How a place is looked at.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    /// Two levels down; new folders and shortcuts at the top are the installer's.
    Normal,
    /// Shared by design (Common Files): nothing is surely one app's.
    Shared,
    /// Two levels down, where only installers write (Start menu folders).
    StartMenu,
    /// Shortcuts only (Desktops): nothing else there is looked at.
    Shortcuts,
    /// New top-level folders only, never surely the installer's (the system drive).
    DriveRoot,
    /// Top-level folders whose names start with a dot (the user's profile).
    DotFolders,
}

struct Root {
    dir: String,
    mode: Mode,
}

fn env(var: &str) -> Option<String> {
    std::env::var(var).ok().filter(|v| !v.trim().is_empty())
}

fn roots() -> Vec<Root> {
    let mut out = Vec::new();
    let mut add = |dir: Option<String>, mode: Mode| {
        if let Some(dir) = dir {
            out.push(Root { dir, mode });
        }
    };
    for var in [
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramData",
        "APPDATA",
        "LOCALAPPDATA",
    ] {
        add(env(var), Mode::Normal);
    }
    for var in ["CommonProgramFiles", "CommonProgramFiles(x86)"] {
        add(env(var), Mode::Shared);
    }
    add(env("LOCALAPPDATA").map(|l| format!(r"{l}\Programs")), Mode::Normal);
    add(
        env("USERPROFILE").map(|p| format!(r"{p}\AppData\LocalLow")),
        Mode::Normal,
    );
    add(env("USERPROFILE"), Mode::DotFolders);
    add(env("SystemDrive").map(|d| format!(r"{d}\")), Mode::DriveRoot);
    let known = imp::known_folders();
    add(known.desktop, Mode::Shortcuts);
    add(known.public_desktop, Mode::Shortcuts);
    add(known.programs, Mode::StartMenu);
    add(known.common_programs, Mode::StartMenu);
    let mut seen = HashSet::new();
    out.retain(|r| seen.insert(canonical(&r.dir)));
    out
}

/// Names Windows itself keeps busy, and Microsoft's own folders, which are
/// never removed anyway.
fn skip_name(name: &str) -> bool {
    const NOISE: [&str; 17] = [
        "temp",
        "tmp",
        "packages",
        "crashdumps",
        "d3dscache",
        "connecteddevicesplatform",
        "comms",
        "peerdistrepub",
        "placeholdertilelogofolder",
        "publishers",
        "package cache",
        "usoshared",
        "usoprivate",
        "virtualstore",
        "history",
        "iconcache.db",
        "desktop.ini",
    ];
    let lower = name.to_lowercase();
    lower.starts_with("microsoft") || lower.starts_with("windows") || NOISE.contains(&lower.as_str())
}

/// Folders Windows keeps at the root of the system drive.
fn skip_drive_name(name: &str) -> bool {
    const SYSTEM: [&str; 9] = [
        "program files",
        "program files (x86)",
        "programdata",
        "users",
        "perflogs",
        "recovery",
        "msocache",
        "onedrivetemp",
        "system volume information",
    ];
    let lower = name.to_lowercase();
    lower.starts_with('$') || lower.starts_with("windows") || SYSTEM.contains(&lower.as_str())
}

fn is_shortcut(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("lnk") || e.eq_ignore_ascii_case("url"))
}

fn path_kind(path: &Path, is_dir: bool) -> &'static str {
    if is_dir {
        "folder"
    } else if is_shortcut(path) {
        "shortcut"
    } else {
        "file"
    }
}

/// A path with a part named like Microsoft's or Windows' own (Edge, Defender…).
fn microsoft_path(path: &str) -> bool {
    norm_path(path)
        .split('\\')
        .skip(1)
        .any(|part| part.starts_with("microsoft") || part.starts_with("windows"))
}

/// Files and folders one or two levels down in every place.
fn look_files(look: &mut Look, own: &[String]) {
    let roots = roots();
    let root_dirs: Vec<String> = roots.iter().map(|r| canonical(&r.dir)).collect();
    let mine = |text: &str| own.iter().any(|o| is_within(o, text) || is_within(text, o));
    for root in &roots {
        let root_key = format!("f:{}", norm_path(&root.dir));
        look.roots.insert(root_key.clone());
        let children = match std::fs::read_dir(&root.dir) {
            Ok(c) => c,
            // Not there yet: the installer may well make it.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                look.unreadable.insert(format!("x:{root_key}"));
                continue;
            }
        };
        for child in children.flatten() {
            let Ok(file_type) = child.file_type() else { continue };
            let name = child.file_name().to_string_lossy().into_owned();
            let path = child.path();
            let text = path.to_string_lossy().into_owned();
            let is_dir = file_type.is_dir();
            // Links lead elsewhere; places of their own are looked at on their own.
            if file_type.is_symlink() || skip_name(&name) || root_dirs.contains(&canonical(&text)) || mine(&text) {
                continue;
            }
            let wanted = match root.mode {
                Mode::Shortcuts => !is_dir && is_shortcut(&path),
                Mode::DriveRoot => is_dir && !skip_drive_name(&name),
                Mode::DotFolders => is_dir && name.starts_with('.'),
                _ => true,
            };
            if !wanted {
                continue;
            }
            let kind = path_kind(&path, is_dir);
            // New folders and shortcuts are an installer's doing; a loose new
            // file may well be someone else's.
            let sure = match root.mode {
                Mode::Normal | Mode::StartMenu => kind != "file",
                Mode::Shortcuts => true,
                Mode::Shared | Mode::DriveRoot | Mode::DotFolders => false,
            };
            let key = format!("f:{}", norm_path(&text));
            look.entries.push(Entry::new(
                key.clone(),
                Some(root_key.clone()),
                kind,
                Target::Path { path: text.clone() },
                sure,
            ));
            let deep = matches!(root.mode, Mode::Normal | Mode::Shared | Mode::StartMenu);
            if !deep || !is_dir {
                continue;
            }
            let inner = match std::fs::read_dir(&path) {
                Ok(i) => i,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => {
                    look.unreadable.insert(format!("x:{key}"));
                    continue;
                }
            };
            for item in inner.flatten() {
                let Ok(item_type) = item.file_type() else { continue };
                let item_name = item.file_name().to_string_lossy().into_owned();
                let item_path = item.path();
                let item_text = item_path.to_string_lossy().into_owned();
                if item_type.is_symlink() || skip_name(&item_name) || mine(&item_text) {
                    continue;
                }
                let item_kind = path_kind(&item_path, item_type.is_dir());
                look.entries.push(Entry::new(
                    format!("f:{}", norm_path(&item_text)),
                    Some(key.clone()),
                    item_kind,
                    Target::Path { path: item_text },
                    root.mode == Mode::StartMenu && item_kind != "file",
                ));
            }
        }
    }
}

/// Services, scheduled tasks, Run entries, PATH, driver packages and installed apps.
fn look_lists(look: &mut Look) {
    // Every service name counts as seen, also the ones whose key can't be
    // opened: they mustn't look new once it can.
    match imp::service_names() {
        Some(names) => look
            .seen
            .extend(names.into_iter().map(|n| format!("s:{}", n.to_lowercase()))),
        None => {
            look.unreadable.insert("x:services".into());
        }
    }
    for s in crate::services::list().unwrap_or_default() {
        if s.is_windows || s.binary.as_deref().is_some_and(microsoft_path) {
            continue;
        }
        let mut entry = Entry::new(format!("s:{}", s.name.to_lowercase()), None, "service", s.removal, true);
        entry.program = s.binary;
        look.entries.push(entry);
    }
    match crate::tasks::list() {
        Ok(tasks) => {
            for t in tasks {
                if t.is_windows || t.binary.as_deref().is_some_and(microsoft_path) {
                    continue;
                }
                let mut entry = Entry::new(format!("t:{}", t.name.to_lowercase()), None, "task", t.removal, true);
                entry.program = t.binary;
                look.entries.push(entry);
            }
        }
        Err(_) => {
            look.unreadable.insert("x:tasks".into());
        }
    }
    // Startup folder shortcuts are files in the Start menu, already looked at.
    for s in crate::startup::list().unwrap_or_default() {
        let Target::RegistryValue { key, name } = &s.removal else {
            continue;
        };
        if s.is_windows || s.target.as_deref().is_some_and(microsoft_path) {
            continue;
        }
        let mut entry = Entry::new(
            format!("r:{}|{}", key.to_lowercase(), name.to_lowercase()),
            None,
            "startup",
            s.removal.clone(),
            true,
        );
        entry.program = s.target.clone();
        look.entries.push(entry);
    }
    for (scope, entry) in crate::orphans::path_entries() {
        look.entries.push(Entry::new(
            format!("p:{scope}|{}", norm_path(&entry)),
            None,
            "environment",
            Target::PathEntry {
                scope: scope.into(),
                entry,
            },
            true,
        ));
    }
    for (key, name) in imp::environment_values() {
        look.entries.push(Entry::new(
            format!("v:{}|{}", key.to_lowercase(), name.to_lowercase()),
            None,
            "environment",
            Target::RegistryValue { key, name },
            false,
        ));
    }
    // By published name only: a package updated under the same name isn't new.
    for (inf, folder) in imp::driver_packages() {
        let original = folder.as_deref().and_then(|f| {
            f.to_lowercase()
                .find(".inf_")
                .and_then(|end| f.get(..end + 4))
                .map(str::to_string)
        });
        let mut entry = Entry::new(
            format!("d:{}", inf.to_lowercase()),
            None,
            "driver",
            Target::Driver { inf: inf.clone() },
            false,
        );
        entry.label = Some(match original {
            Some(o) => format!("Driver package {inf} ({o})"),
            None => format!("Driver package {inf}"),
        });
        entry.store_folder = folder;
        look.entries.push(entry);
    }
    let shared = apps::shared_folders();
    look.apps = apps::list_plain()
        .unwrap_or_default()
        .into_iter()
        .map(|a| AppInfo {
            location: a
                .install_location
                .clone()
                .or_else(|| apps::location_from_uninstaller(a.uninstall_string.as_deref(), &shared))
                .filter(|l| apps::is_own_folder(l, &shared)),
            id: a.id,
            name: a.name,
            publisher: a.publisher,
            version: a.version,
        })
        .collect();
}

fn look(settings: &Settings) -> Look {
    let guard = Guard::new(settings);
    let mut look = Look::default();
    look_files(&mut look, &guard.own);
    imp::look_registry(&mut look);
    look_lists(&mut look);
    look
}

/// A salted hash, so the snapshot on disk doesn't list what's on the PC.
fn hash(salt: &str, text: &str) -> String {
    // FNV-1a: the same in every build, unlike std's hasher.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in salt.bytes().chain([0]).chain(text.bytes()) {
        h ^= u64::from(byte);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn snapshot(look: &Look, salt: &str) -> Snapshot {
    let h = |text: &str| hash(salt, text);
    Snapshot {
        keys: look
            .entries
            .iter()
            .map(|e| h(&e.key))
            .chain(look.seen.iter().map(|k| h(k)))
            .chain(look.unreadable.iter().map(|k| h(k)))
            .collect(),
        roots: look.roots.iter().map(|r| h(r)).collect(),
        apps: look
            .apps
            .iter()
            .map(|a| (h(&a.id), h(a.version.as_deref().unwrap_or_default())))
            .collect(),
        app_names: look.apps.iter().map(|a| h(&a.identity())).collect(),
    }
}

// ---------------------------------------------------------------------------
// Comparing

/// What's new since the snapshot: the topmost new things, nothing inside a
/// place that couldn't be read or wasn't looked at before, and only what was
/// made after tracking started.
fn new_entries(before: &Snapshot, salt: &str, after: &Look, made_since: &dyn Fn(&Entry) -> bool) -> Vec<Entry> {
    let had = |key: &str| before.keys.contains(&hash(salt, key));
    // Places that weren't looked at before (a Desktop OneDrive took over, say)
    // count as unreadable then.
    let unlooked: HashSet<&str> = after
        .roots
        .iter()
        .filter(|r| !before.roots.contains(&hash(salt, r)))
        .map(String::as_str)
        .collect();
    let fresh: HashSet<&str> = after
        .entries
        .iter()
        .filter(|e| !had(&e.key))
        .map(|e| e.key.as_str())
        .collect();
    after
        .entries
        .iter()
        .filter(|e| fresh.contains(e.key.as_str()))
        .filter(|e| match &e.parent {
            // A new folder already covers what's in it.
            Some(p) => !fresh.contains(p.as_str()) && !had(&format!("x:{p}")) && !unlooked.contains(p.as_str()),
            None => true,
        })
        .filter(|e| match e.kind {
            "service" => !had("x:services"),
            "task" => !had("x:tasks"),
            _ => true,
        })
        .filter(|e| made_since(e))
        .cloned()
        .collect()
}

fn is_microsoft(app: &AppInfo) -> bool {
    app.publisher
        .as_deref()
        .is_some_and(|p| p.to_lowercase().starts_with("microsoft"))
}

fn inside(dir: Option<&str>, path: &str) -> bool {
    dir.map(canonical)
        .filter(|d| !d.is_empty())
        .is_some_and(|d| is_within(&canonical(path), &d))
}

/// How many new files and folders lie in an app's folder.
fn evidence(app: &AppInfo, items: &[Entry]) -> usize {
    items
        .iter()
        .filter(|e| matches!(&e.target, Target::Path { path } if inside(app.location.as_deref(), path)))
        .count()
}

/// Ranks apps for "which app was this install for": not Microsoft's, the most
/// new files in its folder, then by name.
fn rank(mut apps: Vec<AppInfo>, items: &[Entry]) -> Vec<AppInfo> {
    apps.sort_by_cached_key(|a| {
        (
            is_microsoft(a),
            std::cmp::Reverse(evidence(a, items)),
            a.name.to_lowercase(),
        )
    });
    apps
}

/// Which apps changed while tracking.
struct Changes {
    /// Installed for the first time.
    fresh: Vec<AppInfo>,
    /// Installed under a new id, but the same app as before (an MSI upgrade, say).
    upgraded: Vec<AppInfo>,
    /// Same id, new version.
    updated: Vec<AppInfo>,
}

fn changes(before: &Snapshot, salt: &str, after: &Look) -> Changes {
    let mut out = Changes {
        fresh: Vec::new(),
        upgraded: Vec::new(),
        updated: Vec::new(),
    };
    for app in &after.apps {
        match before.apps.get(&hash(salt, &app.id)) {
            None if before.app_names.contains(&hash(salt, &app.identity())) => out.upgraded.push(app.clone()),
            None => out.fresh.push(app.clone()),
            Some(version) if *version != hash(salt, app.version.as_deref().unwrap_or_default()) => {
                out.updated.push(app.clone())
            }
            Some(_) => {}
        }
    }
    out
}

/// Names that tie something to an install: the app's, its publisher's and
/// the installer's (without versions), squashed.
fn install_tokens(app: Option<&AppInfo>, installer: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |t: String| {
        if t.len() >= 3 && !out.contains(&t) {
            out.push(t);
        }
    };
    if let Some(app) = app {
        add(squash(&clean_name(&app.name)));
        if let Some(publisher) = app.publisher.as_deref() {
            add(squash(&clean_publisher(publisher)));
        }
        // "Mozilla Firefox" is also "Firefox".
        if let Some((_, rest)) = clean_name(&app.name).split_once(' ') {
            add(squash(rest));
        }
    }
    if let Some(stem) = installer.and_then(|p| Path::new(p).file_stem()) {
        let stem = stem.to_string_lossy().to_lowercase();
        // "OBS-Studio-31.1.2-Windows-Installer" → "obsstudio"; "setup_ravenswatch_1.4" → "ravenswatch".
        let words: Vec<&str> = stem
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty() && !w.chars().all(|c| c.is_ascii_digit()))
            .filter(|w| {
                ![
                    "setup",
                    "install",
                    "installer",
                    "windows",
                    "win",
                    "win64",
                    "x64",
                    "x86",
                    "amd64",
                    "release",
                    "full",
                    "v",
                ]
                .contains(w)
            })
            .collect();
        add(words.concat());
        for w in words {
            add(w.to_string());
        }
    }
    out
}

/// A name matches when it's one of the tokens, or one starts with the other
/// (four letters at least): "JetBrains" and "JetBrains s.r.o.".
fn names_match(name: &str, tokens: &[String]) -> bool {
    let n = squash(&clean_name(name));
    n.len() >= 3
        && tokens
            .iter()
            .any(|t| *t == n || (n.len() >= 4 && t.len() >= 4 && (t.starts_with(&n) || n.starts_with(t.as_str()))))
}

fn installer_name(installer: Option<&str>) -> Option<String> {
    installer
        .and_then(|p| Path::new(p).file_name())
        .map(|n| n.to_string_lossy().into_owned())
}

/// Turns what's new into what the user reviews, for the app chosen (or the
/// likeliest one when `choice` is None; `Some(None)` is "no app").
fn draft(
    info: &Info,
    before: &Snapshot,
    after: &Look,
    new: &[Entry],
    guard: &Guard,
    choice: Option<Option<&str>>,
) -> Draft {
    let changed = changes(before, &info.salt, after);
    // Apps changed by an update only count with new files in their folder:
    // Chrome updating itself in the background isn't what was installed.
    let ranked_fresh = rank(changed.fresh.clone(), new);
    let ranked_other: Vec<AppInfo> = rank(changed.upgraded.iter().chain(&changed.updated).cloned().collect(), new)
        .into_iter()
        .filter(|a| evidence(a, new) > 0)
        .collect();
    let candidates: Vec<AppInfo> = ranked_fresh.iter().chain(&ranked_other).cloned().collect();
    let main: Option<AppInfo> = match choice {
        Some(None) => None,
        Some(Some(id)) => after.apps.iter().find(|a| a.id == id).cloned(),
        None => candidates.first().cloned(),
    };
    let main_id = main.as_ref().map(|a| a.id.clone());
    let also: Vec<AppInfo> = changed
        .fresh
        .iter()
        .filter(|a| Some(&a.id) != main_id.as_ref())
        .cloned()
        .collect();
    // Every other installed app's folder: what's in there is that app's.
    let other_dirs: Vec<String> = after
        .apps
        .iter()
        .filter(|a| Some(&a.id) != main_id.as_ref())
        .filter_map(|a| a.location.as_deref().map(canonical))
        .filter(|d| !d.is_empty())
        .collect();
    let new_dirs: Vec<String> = new
        .iter()
        .filter(|e| e.kind == "folder")
        .filter_map(|e| match &e.target {
            Target::Path { path } => Some(canonical(path)),
            _ => None,
        })
        .collect();
    let main_dir = main.as_ref().and_then(|a| a.location.as_deref()).map(canonical);
    let long_window = now_secs().saturating_sub(info.started_at) > LONG_WINDOW_SECS;

    // What links a new thing to this install, beyond appearing at the same
    // time: the app's names, and where its shortcuts and programs point.
    let tokens = install_tokens(main.as_ref(), info.installer.as_deref());
    let pointed: Vec<String> = new
        .iter()
        .filter_map(|e| match (&e.target, e.kind) {
            (Target::Path { path }, "shortcut") => std::fs::read(path)
                .ok()
                .filter(|b| b.len() < 1 << 20)
                .and_then(|b| crate::orphans::shortcut_target(&b))
                .map(|t| canonical(&t)),
            _ => e.program.as_deref().map(canonical),
        })
        .collect();
    let linked = |e: &Entry| -> bool {
        let last = match &e.target {
            Target::Path { path } | Target::RegistryKey { key: path } => path
                .trim_end_matches('\\')
                .rsplit('\\')
                .next()
                .unwrap_or_default()
                .to_string(),
            _ => return false,
        };
        let stem = last.rsplit_once('.').map_or(last.as_str(), |(s, _)| s);
        if names_match(stem, &tokens) || names_match(&last, &tokens) {
            return true;
        }
        let Target::Path { path } = &e.target else { return false };
        let here = canonical(path);
        main_dir
            .as_deref()
            .is_some_and(|d| !d.is_empty() && (is_within(d, &here) || is_within(&here, d)))
            || pointed.iter().any(|p| is_within(p, &here))
    };
    let mut items: Vec<Entry> = new
        .iter()
        .filter(|e| match &e.target {
            Target::Path { path } => !other_dirs.iter().any(|d| is_within(&canonical(path), d)),
            _ => true,
        })
        .cloned()
        .map(|mut e| {
            if let Some(program) = e.program.as_deref().map(canonical) {
                // A service, task or startup entry is surely the installer's when
                // it runs something the installer added, and never when it runs
                // something of an app that came along.
                let in_also = also.iter().any(|a| inside(a.location.as_deref(), &program));
                e.sure = !in_also
                    && (new_dirs.iter().any(|d| is_within(&program, d))
                        || main_dir
                            .as_deref()
                            .is_some_and(|d| !d.is_empty() && is_within(&program, d)));
            } else if e.sure && matches!(e.kind, "folder" | "file" | "shortcut" | "registry") {
                // Appearing during the install isn't proof on its own: another
                // app may have made it at the same time.
                e.sure = linked(&e);
            }
            if long_window {
                e.sure = false;
            }
            e
        })
        .filter(|e| guard.check(&e.target).is_ok())
        .collect();
    items.sort_by(|a, b| (!a.sure, a.kind, a.key.as_str()).cmp(&(!b.sure, b.kind, b.key.as_str())));

    let sizes = parallel_map(&items, |e| match &e.target {
        Target::Path { path } => Some(dir_stats(Path::new(path)).0),
        _ => None,
    });
    let out: Vec<DraftItem> = items
        .into_iter()
        .zip(sizes)
        .map(|(e, size_bytes)| DraftItem {
            item: TraceItem {
                label: e.label.clone().unwrap_or_else(|| targets::describe(&e.target)),
                id: e.key,
                kind: e.kind.into(),
                target: e.target,
                sure: e.sure,
                store_folder: e.store_folder,
                children: None,
            },
            size_bytes,
        })
        .collect();

    // Its entry in installed apps, when that's new.
    let app_item = main
        .as_ref()
        .filter(|a| !before.apps.contains_key(&hash(&info.salt, &a.id)))
        .and_then(|a| apps::uninstall_key_path(&a.id))
        .map(|key| TraceItem {
            id: format!("a:{}", key.to_lowercase()),
            kind: "app".into(),
            label: key.clone(),
            target: Target::RegistryKey { key },
            sure: true,
            store_folder: None,
            children: None,
        })
        .filter(|item| guard.check(&item.target).is_ok());

    Draft {
        started_at: info.started_at,
        name: main
            .as_ref()
            .map(|a| a.name.clone())
            .or_else(|| {
                installer_name(info.installer.as_deref()).map(|n| {
                    Path::new(&n)
                        .file_stem()
                        .map_or(n.clone(), |s| s.to_string_lossy().into_owned())
                })
            })
            .unwrap_or_else(|| format!("Install of {}", utc_date(info.started_at))),
        app_id: main_id,
        publisher: main.as_ref().and_then(|a| a.publisher.clone()),
        version: main.as_ref().and_then(|a| a.version.clone()),
        updated: main
            .as_ref()
            .is_some_and(|a| !changed.fresh.iter().any(|f| f.id == a.id)),
        also_installed: also.into_iter().map(|a| a.name).collect(),
        candidates: candidates
            .into_iter()
            .map(|a| Candidate { id: a.id, name: a.name })
            .collect(),
        tasks_checked: !before.keys.contains(&hash(&info.salt, "x:tasks")) && !after.unreadable.contains("x:tasks"),
        long_window,
        items: out,
        app_item,
    }
}

// ---------------------------------------------------------------------------
// Storage

fn dir(settings: &Settings) -> PathBuf {
    data_dir(settings, "snapshots")
}

fn info_file(settings: &Settings) -> PathBuf {
    dir(settings).join("tracking.json")
}

fn snapshot_file(settings: &Settings) -> PathBuf {
    dir(settings).join("snapshot.json")
}

fn traces_dir(settings: &Settings) -> PathBuf {
    dir(settings).join("traces")
}

/// Writes next to the file first, so a crash never leaves half of it.
fn write_json<T: Serialize>(file: &Path, value: &T) -> Result<()> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string(value).map_err(|e| Error::Other(e.to_string()))?;
    let temp = file.with_extension("json.new");
    std::fs::write(&temp, json)?;
    std::fs::rename(&temp, file)?;
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(file: &Path) -> Option<T> {
    serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()
}

fn read_info(settings: &Settings) -> Option<Info> {
    read_json(&info_file(settings))
}

fn clear(settings: &Settings) {
    let _ = std::fs::remove_file(info_file(settings));
    let _ = std::fs::remove_file(snapshot_file(settings));
    let mut s = session();
    s.installer = None;
    s.draft = None;
    s.after = None;
}

fn current_user() -> String {
    env("USERPROFILE").map(|p| canonical(&p)).unwrap_or_default()
}

/// Trace ids are what `save_trace` makes: digits, a dash, lowercase letters,
/// digits and dashes. Nothing that could name another file or a device.
fn valid_id(id: &str) -> bool {
    let Some((time, rest)) = id.split_once('-') else {
        return false;
    };
    !time.is_empty()
        && time.bytes().all(|b| b.is_ascii_digit())
        && rest
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn trace_file(settings: &Settings, id: &str) -> Result<PathBuf> {
    if !valid_id(id) {
        return Err(Error::Other("Unknown trace".into()));
    }
    Ok(traces_dir(settings).join(format!("{id}.json")))
}

pub fn list(settings: &Settings) -> Vec<Trace> {
    let mut traces: Vec<Trace> = std::fs::read_dir(traces_dir(settings))
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| {
                    let stem = e.file_name().to_string_lossy().strip_suffix(".json")?.to_string();
                    let trace: Trace = read_json(&e.path())?;
                    (valid_id(&stem) && trace.id == stem).then_some(trace)
                })
                .collect()
        })
        .unwrap_or_default();
    traces.sort_by_key(|t| std::cmp::Reverse(t.finished_at));
    traces
}

fn load(settings: &Settings, id: &str) -> Result<Trace> {
    let trace: Trace = read_json(&trace_file(settings, id)?).ok_or(Error::Other("This trace is gone".into()))?;
    if trace.id != id {
        return Err(Error::Other("This trace can't be read".into()));
    }
    Ok(trace)
}

// ---------------------------------------------------------------------------
// Using a trace later

/// An installed app, as far as matching it to a trace goes.
pub(crate) struct AppLike<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub publisher: Option<&'a str>,
    pub location: Option<&'a str>,
}

/// Whether an installed app is the one a trace belongs to – also after an
/// upgrade gave it a new id.
fn belongs(trace: &Trace, present: &[bool], app: &AppLike) -> bool {
    let Some(app_id) = trace.app_id.as_deref() else {
        return false;
    };
    if app_id == app.id || identity(&trace.name, trace.publisher.as_deref()) == identity(app.name, app.publisher) {
        return true;
    }
    // Its folder is one the install made.
    app.location.is_some_and(|location| {
        trace.items.iter().zip(present).any(|(item, &here)| {
            here && item.kind == "folder"
                && matches!(&item.target, Target::Path { path } if inside(Some(path), location))
        })
    })
}

/// Whether something from a trace is still there – and still the same thing.
fn present(item: &TraceItem) -> bool {
    match &item.target {
        Target::Path { path } => std::fs::symlink_metadata(path).is_ok(),
        Target::PathEntry { scope, entry } => crate::orphans::path_entries()
            .iter()
            .any(|(s, e)| *s == scope.as_str() && norm_path(e) == norm_path(entry)),
        // Windows reuses oem numbers: only the same Driver Store folder is the same package.
        Target::Driver { inf } => item.store_folder.as_deref().is_some_and(|folder| {
            imp::driver_packages().iter().any(|(i, f)| {
                i.eq_ignore_ascii_case(inf) && f.as_deref().is_some_and(|f| f.eq_ignore_ascii_case(folder))
            })
        }),
        target => imp::exists(target),
    }
}

/// What's in a folder or registry key now, lowercase.
fn children_of(target: &Target) -> Option<Vec<String>> {
    match target {
        Target::Path { path } => {
            let entries = std::fs::read_dir(path).ok()?;
            let mut names: Vec<String> = entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().to_lowercase())
                .collect();
            names.sort();
            Some(names)
        }
        Target::RegistryKey { key } => imp::subkeys(key).map(|mut names| {
            names.iter_mut().for_each(|n| *n = n.to_lowercase());
            names.sort();
            names
        }),
        _ => None,
    }
}

fn leftover_kind(kind: &str) -> &'static str {
    match kind {
        "folder" => "folder",
        "shortcut" => "shortcut",
        "registry" => "registry",
        "app" => "app",
        "service" => "service",
        "task" => "task",
        "startup" => "startup",
        "environment" => "environment",
        "driver" => "driver",
        _ => "file",
    }
}

/// Adds what a trace recorded and is still there to a leftover scan. Nothing
/// inside another installed app's folder; a folder or key that holds things
/// the install didn't make only offers what it did make.
pub(crate) fn add_to(found: &mut Found, trace: &Trace, other_folders: &[String]) {
    let date = utc_date(trace.finished_at);
    let elsewhere = |path: &str| other_folders.iter().any(|o| is_within(&canonical(path), o));
    for item in trace.items.iter().filter(|i| present(i)) {
        if matches!(&item.target, Target::Path { path } if elsewhere(path)) {
            continue;
        }
        // An app's data folder that changed since may hold the user's own files.
        let changed_data = item.sure
            && matches!(&item.target, Target::Path { path } if in_data_place(path)
                && dir_stats(Path::new(path)).2.is_some_and(|newest| newest > trace.finished_at + 60));
        let confidence = if item.sure && !changed_data { "high" } else { "medium" };
        let reason = if changed_data {
            format!("Added when it was installed, and changed since – it may hold your data. Tracked {date}")
        } else if item.sure {
            format!("Added when it was installed – tracked {date}")
        } else {
            format!("Appeared while it was installed, but may be another app's – tracked {date}")
        };
        let added_since: Vec<String> = match (&item.children, children_of(&item.target)) {
            (Some(then), Some(now)) => now.into_iter().filter(|n| !then.contains(n)).collect(),
            _ => Vec::new(),
        };
        if added_since.is_empty() {
            found.add(leftover_kind(&item.kind), item.target.clone(), &reason, confidence);
            continue;
        }
        // Shared since: only what was in it when the app was installed.
        let reason =
            format!("Added when it was installed, in a place that now holds other things too – tracked {date}");
        for child in item.children.iter().flatten() {
            match &item.target {
                Target::Path { path } => {
                    let child_path = Path::new(path).join(child);
                    if std::fs::symlink_metadata(&child_path).is_ok() {
                        found.add_path(&child_path, &reason, confidence);
                    }
                }
                Target::RegistryKey { key } => {
                    let child_key = Target::RegistryKey {
                        key: format!(r"{key}\{child}"),
                    };
                    if imp::exists(&child_key) {
                        found.add("registry", child_key, &reason, confidence);
                    }
                }
                _ => {}
            }
        }
    }
}

/// AppData and ProgramData: where apps keep data, rather than their program
/// files (per-user programs in Local\Programs aren't data).
fn in_data_place(path: &str) -> bool {
    let p = canonical(path);
    let programs = env("LOCALAPPDATA").map(|l| canonical(&format!(r"{l}\Programs")));
    if programs.is_some_and(|d| is_within(&p, &d)) {
        return false;
    }
    ["APPDATA", "LOCALAPPDATA", "ProgramData"]
        .iter()
        .filter_map(|v| env(v))
        .chain(env("USERPROFILE").map(|u| format!(r"{u}\AppData\LocalLow")))
        .any(|root| is_within(&p, &canonical(&root)))
}

/// Traces that belong to an installed app, for its leftover scan.
pub(crate) fn traces_for(settings: &Settings, app: &AppLike) -> Vec<Trace> {
    list(settings)
        .into_iter()
        .filter(|t| {
            let present: Vec<bool> = t.items.iter().map(present).collect();
            belongs(t, &present, app)
        })
        .collect()
}

/// Installed apps' own folders, except those of the apps a trace belongs to.
fn other_folders(trace: &Trace, present: &[bool], installed: &[apps::InstalledApp]) -> (Vec<String>, bool) {
    let shared = apps::shared_folders();
    let mut still_installed = false;
    let mut out = Vec::new();
    for a in installed {
        let location = a
            .install_location
            .clone()
            .or_else(|| apps::location_from_uninstaller(a.uninstall_string.as_deref(), &shared))
            .filter(|l| apps::is_own_folder(l, &shared));
        let like = AppLike {
            id: &a.id,
            name: &a.name,
            publisher: a.publisher.as_deref(),
            location: location.as_deref(),
        };
        if belongs(trace, present, &like) {
            still_installed = true;
        } else if let Some(l) = location {
            out.push(canonical(&l));
        }
    }
    (out, still_installed)
}

/// What a trace recorded and is still there, ready to review and remove.
fn leftovers_of(settings: &Settings, trace: &Trace) -> Result<Vec<Leftover>> {
    let installed = apps::list_plain()?;
    let present: Vec<bool> = trace.items.iter().map(present).collect();
    let (others, still_installed) = other_folders(trace, &present, &installed);
    // With no app to tell by, the install may still be in use: nothing is certain.
    let unsure = still_installed || trace.app_id.is_none();
    let mut found = Found::new(Guard::new(settings), others.clone(), unsure);
    add_to(&mut found, trace, &others);
    Ok(found.finish())
}

// ---------------------------------------------------------------------------
// Commands

/// The installer UWiper started, the last look and the last comparison.
struct Session {
    installer: Option<imp::Process>,
    after: Option<(u64, Look)>,
    draft: Option<Draft>,
}

static SESSION: Mutex<Session> = Mutex::new(Session {
    installer: None,
    after: None,
    draft: None,
});

fn session() -> std::sync::MutexGuard<'static, Session> {
    SESSION.lock().unwrap_or_else(|e| e.into_inner())
}

fn status(settings: &Settings) -> TrackingStatus {
    let info = read_info(settings);
    let s = session();
    TrackingStatus {
        active: info.is_some(),
        started_at: info.as_ref().map(|i| i.started_at),
        elevated: info.as_ref().is_some_and(|i| i.elevated),
        installer: info.as_ref().and_then(|i| installer_name(i.installer.as_deref())),
        installer_followed: s.installer.is_some(),
        installer_running: s.installer.as_ref().is_some_and(imp::Process::running),
    }
}

#[tauri::command]
pub async fn tracking_status(state: State<'_, AppState>) -> Result<TrackingStatus> {
    let settings = settings_of(&state);
    blocking(move || Ok(status(&settings))).await
}

/// Takes the "before" snapshot.
#[tauri::command]
pub async fn start_tracking(state: State<'_, AppState>) -> Result<TrackingStatus> {
    let settings = settings_of(&state);
    blocking(move || {
        if !cfg!(windows) {
            return Err(Error::Unsupported);
        }
        if read_info(&settings).is_some() {
            return Err(Error::Other("An install is already being tracked".into()));
        }
        // Before looking: whatever is made during the look counts as new.
        let started_at = now_secs();
        let salt = hash(
            &format!("{:?}", std::time::SystemTime::now()),
            &std::process::id().to_string(),
        );
        let before = look(&settings);
        write_json(&snapshot_file(&settings), &snapshot(&before, &salt))?;
        let info = Info {
            started_at,
            user: current_user(),
            elevated: crate::system::is_elevated(),
            installer: None,
            salt,
        };
        write_json(&info_file(&settings), &info)?;
        let mut s = session();
        s.installer = None;
        s.after = None;
        s.draft = None;
        drop(s);
        Ok(status(&settings))
    })
    .await
}

/// Starts an installer the user picked. It asks for administrator rights
/// itself if it needs them.
#[tauri::command]
pub async fn launch_installer(state: State<'_, AppState>, path: String) -> Result<()> {
    let settings = settings_of(&state);
    blocking(move || {
        let mut info = read_info(&settings).ok_or(Error::Other("Start tracking first".into()))?;
        let file = Path::new(&path);
        let installer = file
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe") || e.eq_ignore_ascii_case("msi"));
        if !installer || !file.is_absolute() || !file.is_file() {
            return Err(Error::Other("Pick an .exe or .msi installer".into()));
        }
        let process = imp::launch(file)?;
        session().installer = process;
        info.installer = Some(path);
        write_json(&info_file(&settings), &info)
    })
    .await
}

fn checked_info(settings: &Settings) -> Result<Info> {
    let info = read_info(settings).ok_or(Error::Other("No install is being tracked".into()))?;
    if info.user != current_user() {
        return Err(Error::Other(
            "Tracking was started by another Windows user – stop it and start again".into(),
        ));
    }
    if info.elevated != crate::system::is_elevated() {
        return Err(Error::Other(if info.elevated {
            "Tracking was started with administrator rights – restart UWiper as administrator to finish it".into()
        } else {
            "Tracking was started without administrator rights – open UWiper normally to finish it, or stop tracking and start again".into()
        }));
    }
    Ok(info)
}

fn compare(settings: &Settings, info: &Info, after: &Look, choice: Option<Option<&str>>) -> Result<Draft> {
    let before: Snapshot = read_json(&snapshot_file(settings)).ok_or(Error::Other(
        "The snapshot can't be read – stop tracking and start again".into(),
    ))?;
    let since = info.started_at.saturating_sub(5);
    let new = new_entries(&before, &info.salt, after, &|e| imp::made_since(e, since));
    Ok(draft(info, &before, after, &new, &Guard::new(settings), choice))
}

/// Takes the "after" snapshot and returns what's new, for review.
#[tauri::command]
pub async fn finish_tracking(state: State<'_, AppState>) -> Result<Draft> {
    let settings = settings_of(&state);
    blocking(move || {
        let info = checked_info(&settings)?;
        let after = look(&settings);
        let result = compare(&settings, &info, &after, None)?;
        // Tracking may have been stopped meanwhile.
        if read_info(&settings).is_some_and(|i| i.started_at == info.started_at) {
            let mut s = session();
            s.after = Some((info.started_at, after));
            s.draft = Some(result.clone());
        }
        Ok(result)
    })
    .await
}

/// Compares again for another app the user picked (`None`: no app).
#[tauri::command]
pub async fn choose_trace_app(state: State<'_, AppState>, app_id: Option<String>) -> Result<Draft> {
    let settings = settings_of(&state);
    blocking(move || {
        let info = checked_info(&settings)?;
        let after = match session().after.clone() {
            Some((started, look)) if started == info.started_at => look,
            _ => return Err(Error::Other("Compare again first".into())),
        };
        let result = compare(&settings, &info, &after, Some(app_id.as_deref()))?;
        session().draft = Some(result.clone());
        Ok(result)
    })
    .await
}

/// Saves the reviewed trace with the items the user kept.
#[tauri::command]
pub async fn save_trace(state: State<'_, AppState>, keep: Vec<String>) -> Result<Trace> {
    let settings = settings_of(&state);
    blocking(move || {
        let info = read_info(&settings).ok_or(Error::Other("No install is being tracked".into()))?;
        let draft = session()
            .draft
            .clone()
            .filter(|d| d.started_at == info.started_at)
            .ok_or(Error::Other("Compare again before saving".into()))?;
        let mut items: Vec<TraceItem> = draft
            .items
            .into_iter()
            .filter(|d| keep.contains(&d.item.id))
            .map(|d| d.item)
            .collect();
        if items.is_empty() {
            return Err(Error::Other("Nothing was kept".into()));
        }
        items.extend(draft.app_item);
        // What's in each new folder and key now; anything else later isn't this app's.
        for item in &mut items {
            if matches!(item.kind.as_str(), "folder" | "registry") {
                item.children = children_of(&item.target);
            }
        }
        let finished_at = now_secs();
        let base = format!("{finished_at}-{}", slug(&draft.name));
        let mut id = base.clone();
        let mut n = 2;
        while traces_dir(&settings).join(format!("{id}.json")).exists() {
            id = format!("{base}-{n}");
            n += 1;
        }
        let trace = Trace {
            id: id.clone(),
            name: draft.name,
            app_id: draft.app_id,
            publisher: draft.publisher,
            version: draft.version,
            installer: installer_name(info.installer.as_deref()),
            started_at: info.started_at,
            finished_at,
            also_installed: draft.also_installed,
            items,
        };
        write_json(&trace_file(&settings, &id)?, &trace)?;
        clear(&settings);
        log::write(
            &settings,
            &[format!(
                "Tracked the install of «{}»: {} items",
                trace.name,
                trace.items.len()
            )],
        );
        Ok(trace)
    })
    .await
}

#[tauri::command]
pub async fn cancel_tracking(state: State<'_, AppState>) -> Result<()> {
    let settings = settings_of(&state);
    blocking(move || {
        clear(&settings);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn list_traces(state: State<'_, AppState>) -> Result<Vec<TraceView>> {
    let settings = settings_of(&state);
    blocking(move || {
        let installed = apps::list_plain().unwrap_or_default();
        Ok(list(&settings)
            .into_iter()
            .map(|trace| {
                let present: Vec<bool> = trace.items.iter().map(present).collect();
                let (_, still_installed) = other_folders(&trace, &present, &installed);
                TraceView {
                    installed: still_installed,
                    present,
                    trace,
                }
            })
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn delete_trace(state: State<'_, AppState>, id: String) -> Result<()> {
    let settings = settings_of(&state);
    blocking(move || {
        let trace = load(&settings, &id)?;
        std::fs::remove_file(trace_file(&settings, &id)?)?;
        log::write(&settings, &[format!("Deleted the install trace of «{}»", trace.name)]);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn trace_leftovers(state: State<'_, AppState>, id: String) -> Result<Vec<Leftover>> {
    let settings = settings_of(&state);
    blocking(move || leftovers_of(&settings, &load(&settings, &id)?)).await
}

// ---------------------------------------------------------------------------
// Windows

#[derive(Default)]
struct KnownFolders {
    desktop: Option<String>,
    public_desktop: Option<String>,
    programs: Option<String>,
    common_programs: Option<String>,
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub struct Process;

    impl Process {
        pub fn running(&self) -> bool {
            false
        }
    }

    pub fn launch(_file: &Path) -> Result<Option<Process>> {
        Err(Error::Unsupported)
    }

    pub fn known_folders() -> KnownFolders {
        KnownFolders::default()
    }

    pub fn look_registry(_look: &mut Look) {}

    pub fn service_names() -> Option<Vec<String>> {
        None
    }

    pub fn environment_values() -> Vec<(String, String)> {
        Vec::new()
    }

    pub fn driver_packages() -> Vec<(String, Option<String>)> {
        Vec::new()
    }

    pub fn subkeys(_key: &str) -> Option<Vec<String>> {
        None
    }

    pub fn exists(_target: &Target) -> bool {
        false
    }

    pub fn made_since(_entry: &Entry, _since: u64) -> bool {
        true
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::targets::{is_published_inf, split_key};
    use crate::util::windows_dir;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, SYSTEMTIME, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Threading::WaitForSingleObject;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY};
    use winreg::RegKey;

    /// An installer UWiper started.
    pub struct Process(HANDLE);

    // The handle is only waited on and closed.
    unsafe impl Send for Process {}

    impl Process {
        pub fn running(&self) -> bool {
            unsafe { WaitForSingleObject(self.0, 0) == WAIT_TIMEOUT }
        }
    }

    impl Drop for Process {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }

    /// Starts the installer. Running as administrator, UWiper hands it to
    /// Explorer instead, so it starts with the user's own rights – an app it
    /// opens at the end shouldn't run elevated – and asks for more itself.
    /// UWiper can't follow it then.
    pub fn launch(file: &Path) -> Result<Option<Process>> {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
        use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        if crate::system::is_elevated() {
            std::process::Command::new(PathBuf::from(windows_dir()).join("explorer.exe"))
                .arg(file)
                .spawn()
                .map_err(|_| Error::Other("The installer didn't start".into()))?;
            return Ok(None);
        }
        let wide = |s: &std::ffi::OsStr| s.encode_wide().chain(Some(0)).collect::<Vec<u16>>();
        let verb = wide("open".as_ref());
        let path = wide(file.as_os_str());
        let folder = wide(file.parent().map_or("".as_ref(), |p| p.as_os_str()));
        let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS;
        info.lpVerb = verb.as_ptr();
        info.lpFile = path.as_ptr();
        info.lpDirectory = folder.as_ptr();
        info.nShow = SW_SHOWNORMAL;
        let ok = unsafe {
            // Shell execution may use COM; this thread is UWiper's own worker.
            CoInitializeEx(
                std::ptr::null(),
                (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
            );
            ShellExecuteExW(&mut info)
        };
        if ok == 0 {
            return Err(Error::Other("The installer didn't start".into()));
        }
        Ok((!info.hProcess.is_null()).then_some(Process(info.hProcess)))
    }

    /// Desktops and Start menus where Windows really keeps them (OneDrive or
    /// another drive, say).
    pub fn known_folders() -> KnownFolders {
        use windows_sys::Win32::System::Com::CoTaskMemFree;
        use windows_sys::Win32::UI::Shell::{
            FOLDERID_CommonPrograms, FOLDERID_Desktop, FOLDERID_Programs, FOLDERID_PublicDesktop, SHGetKnownFolderPath,
        };
        let get = |id: &windows_sys::core::GUID| unsafe {
            let mut raw: windows_sys::core::PWSTR = std::ptr::null_mut();
            let hr = SHGetKnownFolderPath(id, 0, std::ptr::null_mut(), &mut raw);
            let out = (hr >= 0 && !raw.is_null()).then(|| {
                let len = (0..).take_while(|&i| *raw.add(i) != 0).count();
                String::from_utf16_lossy(std::slice::from_raw_parts(raw, len))
            });
            CoTaskMemFree(raw as *const core::ffi::c_void);
            out.filter(|p| !p.is_empty())
        };
        KnownFolders {
            desktop: get(&FOLDERID_Desktop),
            public_desktop: get(&FOLDERID_PublicDesktop),
            programs: get(&FOLDERID_Programs),
            common_programs: get(&FOLDERID_CommonPrograms),
        }
    }

    /// Keys under Software that hold Windows' own settings or shared lists.
    const SHARED_KEYS: [&str; 8] = [
        "microsoft",
        "windows",
        "classes",
        "policies",
        "clients",
        "registeredapplications",
        "wow6432node",
        "appdatalow",
    ];

    fn hive(name: &str) -> RegKey {
        RegKey::predef(if name == "HKLM" {
            HKEY_LOCAL_MACHINE
        } else {
            HKEY_CURRENT_USER
        })
    }

    /// Registry keys one or two levels below a root.
    fn walk(look: &mut Look, hive_name: &str, path: &str, deep: bool) {
        let root_name = format!(r"{hive_name}\{path}");
        let root_key = format!("k:{}", root_name.to_lowercase());
        look.roots.insert(root_key.clone());
        let key = match hive(hive_name).open_subkey_with_flags(path, KEY_READ | KEY_WOW64_64KEY) {
            Ok(k) => k,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(_) => {
                look.unreadable.insert(format!("x:{root_key}"));
                return;
            }
        };
        for name in key.enum_keys().flatten() {
            if deep && SHARED_KEYS.contains(&name.to_lowercase().as_str()) {
                continue;
            }
            let full = format!(r"{root_name}\{name}");
            let entry_key = format!("k:{}", full.to_lowercase());
            look.entries.push(Entry::new(
                entry_key.clone(),
                Some(root_key.clone()),
                "registry",
                Target::RegistryKey { key: full.clone() },
                true,
            ));
            if !deep {
                continue;
            }
            let sub = match key.open_subkey_with_flags(&name, KEY_READ | KEY_WOW64_64KEY) {
                Ok(s) => s,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => {
                    look.unreadable.insert(format!("x:{entry_key}"));
                    continue;
                }
            };
            for inner in sub.enum_keys().flatten() {
                let inner_full = format!(r"{full}\{inner}");
                look.entries.push(Entry::new(
                    format!("k:{}", inner_full.to_lowercase()),
                    Some(entry_key.clone()),
                    "registry",
                    Target::RegistryKey { key: inner_full },
                    false,
                ));
            }
        }
    }

    pub fn look_registry(look: &mut Look) {
        walk(look, "HKCU", "Software", true);
        walk(look, "HKLM", "SOFTWARE", true);
        walk(look, "HKLM", r"SOFTWARE\WOW6432Node", true);
        let app_paths = r"Microsoft\Windows\CurrentVersion\App Paths";
        walk(look, "HKCU", &format!(r"Software\{app_paths}"), false);
        walk(look, "HKLM", &format!(r"SOFTWARE\{app_paths}"), false);
        walk(look, "HKLM", &format!(r"SOFTWARE\WOW6432Node\{app_paths}"), false);
    }

    pub fn service_names() -> Option<Vec<String>> {
        let key = hive("HKLM")
            .open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Services", KEY_READ)
            .ok()?;
        Some(key.enum_keys().flatten().collect())
    }

    const MACHINE_ENV: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";

    /// Environment variables other than PATH and Windows' own, as registry values.
    pub fn environment_values() -> Vec<(String, String)> {
        const WINDOWS: [&str; 14] = [
            "path",
            "pathext",
            "temp",
            "tmp",
            "os",
            "windir",
            "comspec",
            "psmodulepath",
            "driverdata",
            "username",
            "number_of_processors",
            "onedrive",
            "onedriveconsumer",
            "onedrivecommercial",
        ];
        let mut out = Vec::new();
        for (hive_name, path) in [("HKCU", "Environment"), ("HKLM", MACHINE_ENV)] {
            let Ok(key) = hive(hive_name).open_subkey_with_flags(path, KEY_READ) else {
                continue;
            };
            for (name, _) in key.enum_values().flatten() {
                let lower = name.to_lowercase();
                if name.is_empty() || WINDOWS.contains(&lower.as_str()) || lower.starts_with("processor_") {
                    continue;
                }
                out.push((format!(r"{hive_name}\{path}"), name));
            }
        }
        out
    }

    /// Published third-party driver packages with their Driver Store folders.
    pub fn driver_packages() -> Vec<(String, Option<String>)> {
        let Ok(entries) = std::fs::read_dir(PathBuf::from(windows_dir()).join("INF")) else {
            return Vec::new();
        };
        entries
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| is_published_inf(name))
            .map(|name| {
                let folder = crate::drivers::store_location(&name).map(|(folder, _)| folder);
                (name, folder)
            })
            .collect()
    }

    fn open(key: &str) -> std::io::Result<RegKey> {
        let (hive_name, sub) = split_key(key).ok_or(std::io::ErrorKind::NotFound)?;
        hive(hive_name).open_subkey_with_flags(sub, KEY_READ | KEY_WOW64_64KEY)
    }

    pub fn subkeys(key: &str) -> Option<Vec<String>> {
        Some(open(key).ok()?.enum_keys().flatten().collect())
    }

    /// Unreadable counts as there: it may well be.
    fn there(result: std::io::Result<()>) -> bool {
        !matches!(result, Err(e) if e.kind() == std::io::ErrorKind::NotFound)
    }

    fn service_key(name: &str) -> String {
        format!(r"HKLM\SYSTEM\CurrentControlSet\Services\{name}")
    }

    fn task_file(path: &str) -> String {
        format!(r"{}\System32\Tasks{path}", windows_dir())
    }

    pub fn exists(target: &Target) -> bool {
        match target {
            Target::RegistryKey { key } => there(open(key).map(|_| ())),
            Target::RegistryValue { key, name } => there(open(key).and_then(|k| k.get_raw_value(name).map(|_| ()))),
            Target::Service { name } => there(open(&service_key(name)).map(|_| ())),
            Target::Task { path } => there(std::fs::symlink_metadata(task_file(path)).map(|_| ())),
            _ => false,
        }
    }

    /// Unix time of a UTC SYSTEMTIME.
    fn unix(t: &SYSTEMTIME) -> u64 {
        // Days from 1970-01-01 (Howard Hinnant's days_from_civil).
        let (y, m, d) = (i64::from(t.wYear), i64::from(t.wMonth), i64::from(t.wDay));
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146_097 + doe - 719_468;
        let secs = days * 86_400 + i64::from(t.wHour) * 3600 + i64::from(t.wMinute) * 60 + i64::from(t.wSecond);
        u64::try_from(secs).unwrap_or(0)
    }

    fn key_written_since(key: &str, since: u64) -> bool {
        match open(key).and_then(|k| k.query_info()) {
            Ok(meta) => unix(&meta.get_last_write_time_system()) >= since,
            Err(_) => true,
        }
    }

    fn file_made_since(path: &str, since: u64) -> bool {
        std::fs::symlink_metadata(path)
            .and_then(|m| m.created())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .is_none_or(|t| t.as_secs() >= since)
    }

    /// False when the thing was surely there before tracking started: made
    /// (files) or last written (registry keys) earlier. Unknown counts as new.
    pub fn made_since(entry: &Entry, since: u64) -> bool {
        match &entry.target {
            Target::Path { path } => file_made_since(path, since),
            Target::RegistryKey { key } => key_written_since(key, since),
            Target::Service { name } => key_written_since(&service_key(name), since),
            Target::Task { path } => file_made_since(&task_file(path), since),
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, parent: Option<&str>, path: &str, sure: bool) -> Entry {
        Entry::new(
            key.into(),
            parent.map(str::to_string),
            "folder",
            Target::Path { path: path.into() },
            sure,
        )
    }

    const SALT: &str = "salt";

    fn before(keys: &[&str], roots: &[&str]) -> Snapshot {
        Snapshot {
            keys: keys.iter().map(|k| hash(SALT, k)).collect(),
            roots: roots.iter().map(|r| hash(SALT, r)).collect(),
            ..Snapshot::default()
        }
    }

    fn look_of(entries: Vec<Entry>, roots: &[&str]) -> Look {
        Look {
            entries,
            roots: roots.iter().map(|r| r.to_string()).collect(),
            ..Look::default()
        }
    }

    fn keys(entries: Vec<Entry>) -> Vec<String> {
        entries.into_iter().map(|e| e.key).collect()
    }

    #[test]
    fn keeps_only_the_topmost_new_things() {
        let b = before(&["f:c:\\pf\\vendor", "f:c:\\pf\\vendor\\old"], &["f:c:\\pf"]);
        let after = look_of(
            vec![
                entry("f:c:\\pf\\vendor", Some("f:c:\\pf"), r"C:\PF\Vendor", true),
                entry(
                    "f:c:\\pf\\vendor\\old",
                    Some("f:c:\\pf\\vendor"),
                    r"C:\PF\Vendor\Old",
                    false,
                ),
                entry(
                    "f:c:\\pf\\vendor\\new",
                    Some("f:c:\\pf\\vendor"),
                    r"C:\PF\Vendor\New",
                    false,
                ),
                entry("f:c:\\pf\\app", Some("f:c:\\pf"), r"C:\PF\App", true),
                entry("f:c:\\pf\\app\\bin", Some("f:c:\\pf\\app"), r"C:\PF\App\bin", false),
            ],
            &["f:c:\\pf"],
        );
        assert_eq!(
            keys(new_entries(&b, SALT, &after, &|_| true)),
            ["f:c:\\pf\\vendor\\new", "f:c:\\pf\\app"]
        );
        // Made before tracking started: not new, whatever the snapshot says.
        assert_eq!(
            keys(new_entries(&b, SALT, &after, &|e| e.key != "f:c:\\pf\\app")),
            ["f:c:\\pf\\vendor\\new"]
        );
    }

    #[test]
    fn skips_what_couldnt_be_read_or_wasnt_looked_at_before() {
        let b = before(&["x:f:c:\\pd\\locked", "f:c:\\pd\\locked", "x:tasks"], &["f:c:\\pd"]);
        let mut task = entry("t:vendor\\updater", None, "", true);
        task.kind = "task";
        let after = look_of(
            vec![
                entry("f:c:\\pd\\locked", Some("f:c:\\pd"), r"C:\PD\Locked", true),
                entry(
                    "f:c:\\pd\\locked\\inside",
                    Some("f:c:\\pd\\locked"),
                    r"C:\PD\Locked\Inside",
                    false,
                ),
                task,
                // A Desktop that only showed up after tracking started.
                entry(
                    "f:d:\\onedrive\\desktop\\old.lnk",
                    Some("f:d:\\onedrive\\desktop"),
                    r"D:\OneDrive\Desktop\old.lnk",
                    true,
                ),
            ],
            &["f:c:\\pd", "f:d:\\onedrive\\desktop"],
        );
        assert!(new_entries(&b, SALT, &after, &|_| true).is_empty());
    }

    fn app(id: &str, name: &str, publisher: &str, location: Option<&str>) -> AppInfo {
        AppInfo {
            id: id.into(),
            name: name.into(),
            publisher: Some(publisher.into()),
            version: Some("1.0".into()),
            location: location.map(str::to_string),
        }
    }

    #[test]
    fn the_main_app_is_not_a_bundled_runtime() {
        let items = vec![
            entry("f:1", None, r"C:\PF\Game\a", true),
            entry("f:2", None, r"C:\PF\Game\b", true),
            entry("f:3", None, r"C:\PF\Tool\a", true),
        ];
        let ranked: Vec<String> = rank(
            vec![
                app(
                    "1",
                    "Microsoft Visual C++ 2015-2022 Redistributable",
                    "Microsoft Corporation",
                    None,
                ),
                app("2", "Tool", "Vendor", Some(r"C:\PF\Tool")),
                app("3", "Game", "Vendor", Some(r"C:\PF\Game")),
            ],
            &items,
        )
        .into_iter()
        .map(|a| a.name)
        .collect();
        assert_eq!(
            ranked,
            ["Game", "Tool", "Microsoft Visual C++ 2015-2022 Redistributable"]
        );
    }

    #[test]
    fn tells_upgrades_from_new_installs() {
        let old = vec![app("HKLM|0|{OLD}", "Firefox 130.0 (x64 en-US)", "Mozilla", None)];
        let snap = snapshot(
            &Look {
                apps: old,
                ..Look::default()
            },
            SALT,
        );
        let after = Look {
            apps: vec![
                app("HKLM|0|{NEW}", "Firefox 131.0 (x64 en-US)", "Mozilla", None),
                app("HKLM|0|Tool", "Tool", "Vendor", None),
            ],
            ..Look::default()
        };
        let c = changes(&snap, SALT, &after);
        assert_eq!(c.upgraded.len(), 1);
        assert_eq!(c.fresh[0].name, "Tool");
    }

    #[test]
    fn a_trace_belongs_to_its_app_after_an_upgrade() {
        let trace = Trace {
            id: "1-x".into(),
            name: "Firefox 130.0 (x64 en-US)".into(),
            app_id: Some("HKLM|0|{OLD}".into()),
            publisher: Some("Mozilla".into()),
            version: None,
            installer: None,
            started_at: 0,
            finished_at: 0,
            also_installed: vec![],
            items: vec![],
        };
        let upgraded = AppLike {
            id: "HKLM|0|{NEW}",
            name: "Firefox 131.0 (x64 en-US)",
            publisher: Some("Mozilla Corporation"),
            location: None,
        };
        assert!(belongs(&trace, &[], &upgraded));
        let other = AppLike {
            id: "HKLM|0|Thunderbird",
            name: "Thunderbird",
            publisher: Some("Mozilla"),
            location: None,
        };
        assert!(!belongs(&trace, &[], &other));
    }

    #[test]
    fn names_tie_things_to_the_install() {
        let obsidian = app("1", "Obsidian", "Obsidian", None);
        let tokens = install_tokens(Some(&obsidian), Some(r"C:\Downloads\Obsidian-1.7.7.exe"));
        assert!(names_match("obsidian", &tokens));
        assert!(!names_match("Code", &tokens));
        let none = install_tokens(None, Some(r"C:\Downloads\setup_ravenswatch_1.4.exe"));
        assert!(names_match("Ravenswatch", &none));
        assert!(!names_match("Khronos", &none));
        let jb = install_tokens(
            Some(&app("2", "PyCharm Community Edition", "JetBrains s.r.o.", None)),
            None,
        );
        assert!(names_match("JetBrains", &jb));
    }

    #[test]
    fn skips_windows_noise_and_microsoft_folders() {
        for name in [
            "Temp",
            "Packages",
            "Microsoft",
            "Microsoft Edge",
            "WindowsApps",
            "desktop.ini",
            "Package Cache",
        ] {
            assert!(skip_name(name), "{name}");
        }
        for name in ["Discord", "JetBrains", "Steam"] {
            assert!(!skip_name(name), "{name}");
        }
        assert!(skip_drive_name("$Recycle.Bin") && skip_drive_name("Program Files") && !skip_drive_name("xampp"));
        assert!(microsoft_path(
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
        ));
        assert!(!microsoft_path(r"C:\Program Files\Steam\steam.exe"));
    }

    #[test]
    fn trace_ids_stay_inside_the_traces_folder() {
        let settings = Settings::default();
        for bad in [r"..\secrets", "a.b", "COM1", "1700000000-Discord", "-x", "1700000000"] {
            assert!(trace_file(&settings, bad).is_err(), "{bad}");
        }
        assert!(trace_file(&settings, "1700000000-discord-2").is_ok());
    }

    #[test]
    fn hashes_are_salted_and_stable() {
        assert_eq!(hash("a", "x"), hash("a", "x"));
        assert_ne!(hash("a", "x"), hash("b", "x"));
        assert_eq!(hash("", "").len(), 16);
    }
}
