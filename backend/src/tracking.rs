//! Install tracking: a snapshot of the PC before an installer runs and another
//! after it. What's new is saved as the app's trace, and a later uninstall
//! uses it to find everything the installer added.
//!
//! Snapshots and traces hold only names – of files, folders, registry keys,
//! services and the like – never anything inside them, and stay in UWiper's
//! own folder.
//!
//! Each place is looked at two levels down: a new folder in Program Files, or
//! a new folder inside a vendor's folder that was already there. Anything
//! deeper inside a folder that already existed is left out, since other apps
//! running at the same time write there too.
#![cfg_attr(not(windows), allow(dead_code))]

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{blocking, Error, Result};
use crate::leftovers::{Found, Leftover};
use crate::paths::data_dir;
use crate::quarantine::{settings_of, slug};
use crate::settings::Settings;
use crate::targets::{self, Guard, Target};
use crate::util::{canonical, dir_stats, is_within, norm_path, now_secs, parallel_map, utc_date};
use crate::{apps, log, AppState};

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
    /// Made at the top of its place – a new folder in Program Files, a new key
    /// under Software – rather than inside something that was already there.
    sure: bool,
    /// For a driver package: its Driver Store folder, which tells it apart
    /// from a later package Windows publishes under the same oem name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    store_folder: Option<String>,
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

/// A snapshot taken before installing, kept on disk so tracking survives a
/// restart of UWiper or of Windows.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Pending {
    started_at: u64,
    /// The Windows user it was taken for: another user's AppData would all look new.
    user: String,
    installer: Option<String>,
    keys: BTreeSet<String>,
    /// Installed apps and their versions.
    apps: BTreeMap<String, String>,
}

/// What `finish_tracking` found, for the user to look over before saving.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    name: String,
    app_id: Option<String>,
    publisher: Option<String>,
    version: Option<String>,
    /// The app was already installed and this was an update.
    updated: bool,
    also_installed: Vec<String>,
    /// False when scheduled tasks couldn't be read (without administrator rights).
    tasks_checked: bool,
    items: Vec<DraftItem>,
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
    installer: Option<String>,
    /// The installer UWiper started is still running.
    installer_running: bool,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TraceView {
    #[serde(flatten)]
    trace: Trace,
    /// Its app is still installed.
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
    sure: bool,
    store_folder: Option<String>,
}

#[derive(Clone, Debug)]
struct AppInfo {
    id: String,
    name: String,
    publisher: Option<String>,
    version: Option<String>,
    location: Option<String>,
}

#[derive(Debug, Default)]
struct Look {
    entries: Vec<Entry>,
    /// Places that couldn't be read (`x:` keys). Nothing in them counts as new
    /// later, since it may have been there all along.
    unreadable: BTreeSet<String>,
    apps: Vec<AppInfo>,
}

impl Look {
    fn keys(&self) -> BTreeSet<String> {
        self.entries
            .iter()
            .map(|e| e.key.clone())
            .chain(self.unreadable.iter().cloned())
            .collect()
    }

    fn app_versions(&self) -> BTreeMap<String, String> {
        self.apps
            .iter()
            .map(|a| (a.id.clone(), a.version.clone().unwrap_or_default()))
            .collect()
    }
}

/// A place to look at, one or two levels down.
struct Root {
    dir: String,
    deep: bool,
    /// Whatever appears inside its existing folders is the installer's too
    /// (Start menu folders: apps don't write there while running).
    sure_inside: bool,
}

fn env(var: &str) -> Option<String> {
    std::env::var(var).ok().filter(|v| !v.trim().is_empty())
}

fn roots() -> Vec<Root> {
    let mut out = Vec::new();
    let mut add = |dir: String, deep: bool, sure_inside: bool| out.push(Root { dir, deep, sure_inside });
    for var in [
        "ProgramFiles",
        "ProgramFiles(x86)",
        "CommonProgramFiles",
        "CommonProgramFiles(x86)",
        "ProgramData",
        "APPDATA",
        "LOCALAPPDATA",
    ] {
        if let Some(dir) = env(var) {
            add(dir, true, false);
        }
    }
    if let Some(local) = env("LOCALAPPDATA") {
        add(format!(r"{local}\Programs"), true, false);
    }
    if let Some(profile) = env("USERPROFILE") {
        add(format!(r"{profile}\AppData\LocalLow"), true, false);
        add(format!(r"{profile}\Desktop"), false, false);
    }
    if let Some(public) = env("PUBLIC") {
        add(format!(r"{public}\Desktop"), false, false);
    }
    for var in ["APPDATA", "ProgramData"] {
        if let Some(dir) = env(var) {
            add(format!(r"{dir}\Microsoft\Windows\Start Menu\Programs"), true, true);
        }
    }
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

fn path_kind(path: &Path, is_dir: bool) -> &'static str {
    if is_dir {
        "folder"
    } else if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("lnk") || e.eq_ignore_ascii_case("url"))
    {
        "shortcut"
    } else {
        "file"
    }
}

/// Files and folders one or two levels down in every place.
fn look_files(look: &mut Look, own: &[String]) {
    let roots = roots();
    let root_dirs: Vec<String> = roots.iter().map(|r| canonical(&r.dir)).collect();
    for root in &roots {
        let root_key = format!("f:{}", norm_path(&root.dir));
        let Ok(children) = std::fs::read_dir(&root.dir) else {
            look.unreadable.insert(format!("x:{root_key}"));
            continue;
        };
        for child in children.flatten() {
            let Ok(file_type) = child.file_type() else { continue };
            let name = child.file_name().to_string_lossy().into_owned();
            let path = child.path();
            let text = path.to_string_lossy().into_owned();
            // Links lead elsewhere; places of their own are looked at on their own.
            if file_type.is_symlink() || skip_name(&name) || root_dirs.contains(&canonical(&text)) {
                continue;
            }
            if own.iter().any(|o| is_within(o, &text) || is_within(&text, o)) {
                continue;
            }
            let key = format!("f:{}", norm_path(&text));
            look.entries.push(Entry {
                key: key.clone(),
                parent: Some(root_key.clone()),
                kind: path_kind(&path, file_type.is_dir()),
                target: Target::Path { path: text.clone() },
                sure: true,
                store_folder: None,
            });
            if !root.deep || !file_type.is_dir() {
                continue;
            }
            let Ok(inner) = std::fs::read_dir(&path) else {
                look.unreadable.insert(format!("x:{key}"));
                continue;
            };
            for item in inner.flatten() {
                let Ok(item_type) = item.file_type() else { continue };
                let item_name = item.file_name().to_string_lossy().into_owned();
                if item_type.is_symlink() || skip_name(&item_name) {
                    continue;
                }
                let item_path = item.path();
                let item_text = item_path.to_string_lossy().into_owned();
                if own.iter().any(|o| is_within(o, &item_text) || is_within(&item_text, o)) {
                    continue;
                }
                look.entries.push(Entry {
                    key: format!("f:{}", norm_path(&item_text)),
                    parent: Some(key.clone()),
                    kind: path_kind(&item_path, item_type.is_dir()),
                    target: Target::Path { path: item_text },
                    sure: root.sure_inside,
                    store_folder: None,
                });
            }
        }
    }
}

/// Services, scheduled tasks, Run entries, PATH, driver packages and installed apps.
fn look_lists(look: &mut Look) {
    match crate::services::list() {
        Ok(services) => {
            for s in services.into_iter().filter(|s| !s.is_windows) {
                look.entries.push(Entry {
                    key: format!("s:{}", s.name.to_lowercase()),
                    parent: None,
                    kind: "service",
                    target: s.removal,
                    sure: true,
                    store_folder: None,
                });
            }
        }
        Err(_) => {
            look.unreadable.insert("x:services".into());
        }
    }
    match crate::tasks::list() {
        Ok(tasks) => {
            for t in tasks.into_iter().filter(|t| !t.is_windows) {
                look.entries.push(Entry {
                    key: format!("t:{}", t.name.to_lowercase()),
                    parent: None,
                    kind: "task",
                    target: t.removal,
                    sure: true,
                    store_folder: None,
                });
            }
        }
        Err(_) => {
            look.unreadable.insert("x:tasks".into());
        }
    }
    // Startup folder shortcuts are files in the Start menu, already looked at.
    for s in crate::startup::list().unwrap_or_default() {
        if let Target::RegistryValue { key, name } = &s.removal {
            if s.is_windows {
                continue;
            }
            look.entries.push(Entry {
                key: format!("r:{}|{}", key.to_lowercase(), name.to_lowercase()),
                parent: None,
                kind: "startup",
                target: s.removal.clone(),
                sure: true,
                store_folder: None,
            });
        }
    }
    for (scope, entry) in crate::orphans::path_entries() {
        look.entries.push(Entry {
            key: format!("p:{scope}|{}", norm_path(&entry)),
            parent: None,
            kind: "environment",
            target: Target::PathEntry {
                scope: scope.into(),
                entry,
            },
            sure: true,
            store_folder: None,
        });
    }
    for (inf, folder) in imp::driver_packages() {
        look.entries.push(Entry {
            key: format!(
                "d:{}|{}",
                inf.to_lowercase(),
                folder.as_deref().unwrap_or_default().to_lowercase()
            ),
            parent: None,
            kind: "driver",
            target: Target::Driver { inf },
            sure: true,
            store_folder: folder,
        });
    }
    look.apps = apps::list_plain()
        .unwrap_or_default()
        .into_iter()
        .map(|a| AppInfo {
            id: a.id,
            name: a.name,
            publisher: a.publisher,
            version: a.version,
            location: a.install_location,
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

// ---------------------------------------------------------------------------
// Comparing

/// What's new since the snapshot: the topmost new things, nothing inside a
/// place that couldn't be read before.
fn new_entries(before: &BTreeSet<String>, after: &Look) -> Vec<Entry> {
    let fresh: HashSet<&str> = after
        .entries
        .iter()
        .filter(|e| !before.contains(&e.key))
        .map(|e| e.key.as_str())
        .collect();
    after
        .entries
        .iter()
        .filter(|e| fresh.contains(e.key.as_str()))
        .filter(|e| match &e.parent {
            // A new folder already covers what's in it.
            Some(p) => !fresh.contains(p.as_str()) && !before.contains(&format!("x:{p}")),
            None => true,
        })
        .filter(|e| match e.kind {
            "service" => !before.contains("x:services"),
            "task" => !before.contains("x:tasks"),
            _ => true,
        })
        .cloned()
        .collect()
}

fn is_microsoft(app: &AppInfo) -> bool {
    app.publisher
        .as_deref()
        .is_some_and(|p| p.to_lowercase().starts_with("microsoft"))
}

/// The app this install was for, and the others that came along with it
/// (runtimes and the like). The main one isn't Microsoft's and has the most
/// new files in its folder.
fn choose_app(new_apps: Vec<AppInfo>, items: &[Entry]) -> (Option<AppInfo>, Vec<AppInfo>) {
    let inside = |app: &AppInfo| {
        let Some(dir) = app.location.as_deref().map(canonical).filter(|d| !d.is_empty()) else {
            return 0;
        };
        items
            .iter()
            .filter(|e| matches!(&e.target, Target::Path { path } if is_within(&canonical(path), &dir)))
            .count()
    };
    let mut ranked: Vec<(bool, std::cmp::Reverse<usize>, String, AppInfo)> = new_apps
        .into_iter()
        .map(|a| {
            (
                is_microsoft(&a),
                std::cmp::Reverse(inside(&a)),
                a.name.to_lowercase(),
                a,
            )
        })
        .collect();
    ranked.sort_by(|a, b| (a.0, a.1, &a.2).cmp(&(b.0, b.1, &b.2)));
    let mut apps = ranked.into_iter().map(|r| r.3);
    let main = apps.next();
    (main, apps.collect())
}

fn installer_name(installer: Option<&str>) -> Option<String> {
    installer
        .and_then(|p| Path::new(p).file_name())
        .map(|n| n.to_string_lossy().into_owned())
}

/// Turns the difference between two looks into what the user reviews.
fn draft(pending: &Pending, after: &Look, guard: &Guard) -> Draft {
    let mut items = new_entries(&pending.keys, after);
    let new_apps: Vec<AppInfo> = after
        .apps
        .iter()
        .filter(|a| !pending.apps.contains_key(&a.id))
        .cloned()
        .collect();
    let updated: Option<AppInfo> = after
        .apps
        .iter()
        .find(|a| {
            pending
                .apps
                .get(&a.id)
                .is_some_and(|v| *v != a.version.clone().unwrap_or_default())
        })
        .cloned();
    let (main, also) = choose_app(new_apps, &items);

    // What belongs to the apps that came along stays theirs.
    let also_dirs: Vec<String> = also
        .iter()
        .filter_map(|a| a.location.as_deref().map(canonical))
        .filter(|d| !d.is_empty() && guard.check(&Target::Path { path: d.clone() }).is_ok())
        .collect();
    items.retain(|e| match &e.target {
        Target::Path { path } => !also_dirs.iter().any(|d| is_within(&canonical(path), d)),
        _ => true,
    });
    if let Some(key) = main.as_ref().and_then(|a| apps::uninstall_key_path(&a.id)) {
        items.push(Entry {
            key: format!("a:{}", key.to_lowercase()),
            parent: None,
            kind: "app",
            target: Target::RegistryKey { key },
            sure: true,
            store_folder: None,
        });
    }
    items.retain(|e| guard.check(&e.target).is_ok());

    let sizes = parallel_map(&items, |e| match &e.target {
        Target::Path { path } => Some(dir_stats(Path::new(path)).0),
        _ => None,
    });
    let mut out: Vec<DraftItem> = items
        .into_iter()
        .zip(sizes)
        .map(|(e, size_bytes)| DraftItem {
            item: TraceItem {
                label: targets::describe(&e.target),
                id: e.key,
                kind: e.kind.into(),
                target: e.target,
                sure: e.sure,
                store_folder: e.store_folder,
            },
            size_bytes,
        })
        .collect();
    out.sort_by(|a, b| {
        (!a.item.sure, &a.item.kind, a.item.label.to_lowercase()).cmp(&(
            !b.item.sure,
            &b.item.kind,
            b.item.label.to_lowercase(),
        ))
    });

    let app = main.clone().or_else(|| updated.clone());
    Draft {
        name: app
            .as_ref()
            .map(|a| a.name.clone())
            .or_else(|| {
                installer_name(pending.installer.as_deref()).map(|n| {
                    Path::new(&n)
                        .file_stem()
                        .map_or(n.clone(), |s| s.to_string_lossy().into_owned())
                })
            })
            .unwrap_or_else(|| format!("Install of {}", utc_date(pending.started_at))),
        app_id: app.as_ref().map(|a| a.id.clone()),
        publisher: app.as_ref().and_then(|a| a.publisher.clone()),
        version: app.as_ref().and_then(|a| a.version.clone()),
        updated: main.is_none() && updated.is_some(),
        also_installed: also.into_iter().map(|a| a.name).collect(),
        tasks_checked: !pending.keys.contains("x:tasks") && !after.unreadable.contains("x:tasks"),
        items: out,
    }
}

// ---------------------------------------------------------------------------
// Storage

fn dir(settings: &Settings) -> PathBuf {
    data_dir(settings, "snapshots")
}

fn pending_file(settings: &Settings) -> PathBuf {
    dir(settings).join("pending.json")
}

fn traces_dir(settings: &Settings) -> PathBuf {
    dir(settings).join("traces")
}

/// Writes next to the file first, so a crash never leaves half of it.
fn write_json<T: Serialize>(file: &Path, value: &T) -> Result<()> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value).map_err(|e| Error::Other(e.to_string()))?;
    let temp = file.with_extension("json.new");
    std::fs::write(&temp, json)?;
    std::fs::rename(&temp, file)?;
    Ok(())
}

fn read_pending(settings: &Settings) -> Option<Pending> {
    serde_json::from_str(&std::fs::read_to_string(pending_file(settings)).ok()?).ok()
}

fn current_user() -> String {
    env("USERPROFILE").map(|p| canonical(&p)).unwrap_or_default()
}

pub fn list(settings: &Settings) -> Vec<Trace> {
    let mut traces: Vec<Trace> = std::fs::read_dir(traces_dir(settings))
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
                .filter_map(|e| serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok())
                .collect()
        })
        .unwrap_or_default();
    traces.sort_by_key(|t: &Trace| std::cmp::Reverse(t.finished_at));
    traces
}

fn trace_file(settings: &Settings, id: &str) -> Result<PathBuf> {
    if id.is_empty() || id.contains(['/', '\\', ':', '.']) {
        return Err(Error::Other("Unknown trace".into()));
    }
    Ok(traces_dir(settings).join(format!("{id}.json")))
}

fn load(settings: &Settings, id: &str) -> Result<Trace> {
    let text =
        std::fs::read_to_string(trace_file(settings, id)?).map_err(|_| Error::Other("This trace is gone".into()))?;
    serde_json::from_str(&text).map_err(|_| Error::Other("This trace can't be read".into()))
}

// ---------------------------------------------------------------------------
// Using a trace later

/// Whether something from a trace is still there – and still the same thing.
fn present(item: &TraceItem) -> bool {
    match &item.target {
        Target::Path { path } => std::fs::symlink_metadata(path).is_ok(),
        Target::PathEntry { scope, entry } => crate::orphans::path_entries()
            .iter()
            .any(|(s, e)| *s == scope.as_str() && norm_path(e) == norm_path(entry)),
        Target::Driver { inf } => imp::driver_packages().iter().any(|(i, folder)| {
            i.eq_ignore_ascii_case(inf)
                && (item.store_folder.is_none()
                    || folder.as_deref().map(str::to_lowercase) == item.store_folder.as_deref().map(str::to_lowercase))
        }),
        target => imp::exists(target),
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

/// Adds what a trace recorded and is still there to a leftover scan.
pub(crate) fn add_to(found: &mut Found, trace: &Trace) {
    let date = utc_date(trace.finished_at);
    for item in trace.items.iter().filter(|i| present(i)) {
        let (reason, confidence) = if item.sure {
            (format!("Added when it was installed – tracked {date}"), "high")
        } else {
            (
                format!("Added inside a folder that was already there while it was installed – tracked {date}"),
                "medium",
            )
        };
        found.add(leftover_kind(&item.kind), item.target.clone(), &reason, confidence);
    }
}

/// Traces of an installed app, for its leftover scan.
pub(crate) fn traces_for(settings: &Settings, app_id: &str) -> Vec<Trace> {
    list(settings)
        .into_iter()
        .filter(|t| t.app_id.as_deref() == Some(app_id))
        .collect()
}

/// What a trace recorded and is still there, ready to review and remove.
fn leftovers_of(settings: &Settings, trace: &Trace) -> Result<Vec<Leftover>> {
    let installed = apps::list_plain()?;
    let still_installed = trace
        .app_id
        .as_deref()
        .is_some_and(|id| installed.iter().any(|a| a.id == id));
    let shared = apps::shared_folders();
    let other_folders: Vec<String> = installed
        .iter()
        .filter(|a| Some(a.id.as_str()) != trace.app_id.as_deref())
        .filter_map(|a| {
            a.install_location
                .clone()
                .or_else(|| apps::location_from_uninstaller(a.uninstall_string.as_deref(), &shared))
        })
        .map(|p| canonical(&p))
        .filter(|p| !p.is_empty() && apps::is_own_folder(p, &shared))
        .collect();
    let mut found = Found::new(Guard::new(settings), other_folders, still_installed);
    add_to(&mut found, trace);
    Ok(found.finish())
}

// ---------------------------------------------------------------------------
// Commands

/// The installer UWiper started, and the last comparison.
struct Session {
    installer: Option<imp::Process>,
    draft: Option<Draft>,
}

static SESSION: Mutex<Session> = Mutex::new(Session {
    installer: None,
    draft: None,
});

fn session() -> std::sync::MutexGuard<'static, Session> {
    SESSION.lock().unwrap_or_else(|e| e.into_inner())
}

fn status(settings: &Settings) -> TrackingStatus {
    let pending = read_pending(settings);
    TrackingStatus {
        active: pending.is_some(),
        started_at: pending.as_ref().map(|p| p.started_at),
        installer: pending.as_ref().and_then(|p| installer_name(p.installer.as_deref())),
        installer_running: session().installer.as_ref().is_some_and(imp::Process::running),
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
        if read_pending(&settings).is_some() {
            return Err(Error::Other("An install is already being tracked".into()));
        }
        let before = look(&settings);
        let pending = Pending {
            started_at: now_secs(),
            user: current_user(),
            installer: None,
            keys: before.keys(),
            apps: before.app_versions(),
        };
        write_json(&pending_file(&settings), &pending)?;
        let mut s = session();
        s.installer = None;
        s.draft = None;
        drop(s);
        Ok(status(&settings))
    })
    .await
}

/// Starts an installer the user picked, with its own UAC prompt if it asks for one.
#[tauri::command]
pub async fn launch_installer(state: State<'_, AppState>, path: String) -> Result<()> {
    let settings = settings_of(&state);
    blocking(move || {
        let mut pending = read_pending(&settings).ok_or(Error::Other("Start tracking first".into()))?;
        let file = Path::new(&path);
        let installer = file
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe") || e.eq_ignore_ascii_case("msi"));
        if !installer || !file.is_absolute() || !file.is_file() {
            return Err(Error::Other("Pick an .exe or .msi installer".into()));
        }
        let process = imp::launch(file)?;
        session().installer = Some(process);
        pending.installer = Some(path);
        write_json(&pending_file(&settings), &pending)
    })
    .await
}

/// Takes the "after" snapshot and returns what's new, for review.
#[tauri::command]
pub async fn finish_tracking(state: State<'_, AppState>) -> Result<Draft> {
    let settings = settings_of(&state);
    blocking(move || {
        let pending = read_pending(&settings).ok_or(Error::Other("No install is being tracked".into()))?;
        if pending.user != current_user() {
            return Err(Error::Other(
                "Tracking was started by another Windows user – cancel it and start again".into(),
            ));
        }
        let after = look(&settings);
        let result = draft(&pending, &after, &Guard::new(&settings));
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
        let pending = read_pending(&settings).ok_or(Error::Other("No install is being tracked".into()))?;
        let draft = session()
            .draft
            .clone()
            .ok_or(Error::Other("Compare again before saving".into()))?;
        let items: Vec<TraceItem> = draft
            .items
            .into_iter()
            .filter(|d| keep.contains(&d.item.id))
            .map(|d| d.item)
            .collect();
        if items.is_empty() {
            return Err(Error::Other("Nothing was kept".into()));
        }
        let finished_at = now_secs();
        let mut id = format!("{finished_at}-{}", slug(&draft.name));
        let mut n = 2;
        while traces_dir(&settings).join(format!("{id}.json")).exists() {
            id = format!("{finished_at}-{}-{n}", slug(&draft.name));
            n += 1;
        }
        let trace = Trace {
            id: id.clone(),
            name: draft.name,
            app_id: draft.app_id,
            publisher: draft.publisher,
            version: draft.version,
            installer: installer_name(pending.installer.as_deref()),
            started_at: pending.started_at,
            finished_at,
            also_installed: draft.also_installed,
            items,
        };
        write_json(&trace_file(&settings, &id)?, &trace)?;
        let _ = std::fs::remove_file(pending_file(&settings));
        end_session();
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

fn end_session() {
    let mut s = session();
    s.installer = None;
    s.draft = None;
}

#[tauri::command]
pub async fn cancel_tracking(state: State<'_, AppState>) -> Result<()> {
    let settings = settings_of(&state);
    blocking(move || {
        let _ = std::fs::remove_file(pending_file(&settings));
        end_session();
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn list_traces(state: State<'_, AppState>) -> Result<Vec<TraceView>> {
    let settings = settings_of(&state);
    blocking(move || {
        let installed: HashSet<String> = apps::list_plain()
            .unwrap_or_default()
            .into_iter()
            .map(|a| a.id)
            .collect();
        Ok(list(&settings)
            .into_iter()
            .map(|trace| TraceView {
                installed: trace.app_id.as_ref().is_some_and(|id| installed.contains(id)),
                present: trace.items.iter().map(present).collect(),
                trace,
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

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub struct Process;

    impl Process {
        pub fn running(&self) -> bool {
            false
        }
    }

    pub fn launch(_file: &Path) -> Result<Process> {
        Err(Error::Unsupported)
    }

    pub fn look_registry(_look: &mut Look) {}

    pub fn driver_packages() -> Vec<(String, Option<String>)> {
        Vec::new()
    }

    pub fn exists(_target: &Target) -> bool {
        false
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::targets::{is_published_inf, split_key};
    use crate::util::windows_dir;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_TIMEOUT};
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

    pub fn launch(file: &Path) -> Result<Process> {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

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
        if unsafe { ShellExecuteExW(&mut info) } == 0 {
            return Err(Error::Other("The installer didn't start".into()));
        }
        if info.hProcess.is_null() {
            return Err(Error::Other(
                "The installer started, but UWiper can't follow it – click Done when it's finished".into(),
            ));
        }
        Ok(Process(info.hProcess))
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

    /// Registry keys one or two levels below a root.
    fn walk(look: &mut Look, hive: &str, path: &str, deep: bool) {
        let root = RegKey::predef(if hive == "HKLM" {
            HKEY_LOCAL_MACHINE
        } else {
            HKEY_CURRENT_USER
        });
        let root_name = format!(r"{hive}\{path}");
        let root_key = format!("k:{}", root_name.to_lowercase());
        let Ok(key) = root.open_subkey_with_flags(path, KEY_READ | KEY_WOW64_64KEY) else {
            look.unreadable.insert(format!("x:{root_key}"));
            return;
        };
        for name in key.enum_keys().flatten() {
            if deep && SHARED_KEYS.contains(&name.to_lowercase().as_str()) {
                continue;
            }
            let full = format!(r"{root_name}\{name}");
            let entry_key = format!("k:{}", full.to_lowercase());
            look.entries.push(Entry {
                key: entry_key.clone(),
                parent: Some(root_key.clone()),
                kind: "registry",
                target: Target::RegistryKey { key: full.clone() },
                sure: true,
                store_folder: None,
            });
            if !deep {
                continue;
            }
            let Ok(sub) = key.open_subkey_with_flags(&name, KEY_READ | KEY_WOW64_64KEY) else {
                look.unreadable.insert(format!("x:{entry_key}"));
                continue;
            };
            for inner in sub.enum_keys().flatten() {
                let inner_full = format!(r"{full}\{inner}");
                look.entries.push(Entry {
                    key: format!("k:{}", inner_full.to_lowercase()),
                    parent: Some(entry_key.clone()),
                    kind: "registry",
                    target: Target::RegistryKey { key: inner_full },
                    sure: false,
                    store_folder: None,
                });
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

    /// Published third-party driver packages with their Driver Store folders.
    pub fn driver_packages() -> Vec<(String, Option<String>)> {
        let database = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags(r"SYSTEM\DriverDatabase\DriverInfFiles", KEY_READ)
            .ok();
        let Ok(entries) = std::fs::read_dir(PathBuf::from(windows_dir()).join("INF")) else {
            return Vec::new();
        };
        entries
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| is_published_inf(name))
            .map(|name| {
                let folder = database.as_ref().and_then(|db| {
                    let key = db.open_subkey_with_flags(&name, KEY_READ).ok()?;
                    key.get_value::<String, _>("Active")
                        .ok()
                        .or_else(|| key.get_value::<Vec<String>, _>("").ok()?.into_iter().next())
                        .filter(|f| !f.is_empty())
                });
                (name, folder)
            })
            .collect()
    }

    fn open(key: &str) -> std::io::Result<RegKey> {
        let (hive, sub) = split_key(key).ok_or(std::io::ErrorKind::NotFound)?;
        let root = RegKey::predef(if hive == "HKLM" {
            HKEY_LOCAL_MACHINE
        } else {
            HKEY_CURRENT_USER
        });
        root.open_subkey_with_flags(sub, KEY_READ | KEY_WOW64_64KEY)
    }

    /// Unreadable counts as there: it may well be.
    fn there(result: std::io::Result<()>) -> bool {
        !matches!(result, Err(e) if e.kind() == std::io::ErrorKind::NotFound)
    }

    pub fn exists(target: &Target) -> bool {
        match target {
            Target::RegistryKey { key } => there(open(key).map(|_| ())),
            Target::RegistryValue { key, name } => there(open(key).and_then(|k| k.get_raw_value(name).map(|_| ()))),
            Target::Service { name } => {
                there(open(&format!(r"HKLM\SYSTEM\CurrentControlSet\Services\{name}")).map(|_| ()))
            }
            Target::Task { path } => {
                there(std::fs::symlink_metadata(format!(r"{}\System32\Tasks{path}", windows_dir())).map(|_| ()))
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, parent: Option<&str>, path: &str, sure: bool) -> Entry {
        Entry {
            key: key.into(),
            parent: parent.map(str::to_string),
            kind: "folder",
            target: Target::Path { path: path.into() },
            sure,
            store_folder: None,
        }
    }

    fn look_of(entries: Vec<Entry>) -> Look {
        Look {
            entries,
            ..Look::default()
        }
    }

    #[test]
    fn keeps_only_the_topmost_new_things() {
        let before: BTreeSet<String> = ["f:c:\\pf\\vendor", "f:c:\\pf\\vendor\\old"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let after = look_of(vec![
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
        ]);
        let new: Vec<String> = new_entries(&before, &after).into_iter().map(|e| e.key).collect();
        assert_eq!(new, ["f:c:\\pf\\vendor\\new", "f:c:\\pf\\app"]);
    }

    #[test]
    fn skips_what_couldnt_be_read_before() {
        let before: BTreeSet<String> = ["x:f:c:\\pd\\locked", "f:c:\\pd\\locked", "x:tasks"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut task = entry("t:vendor\\updater", None, "", true);
        task.kind = "task";
        let after = look_of(vec![
            entry("f:c:\\pd\\locked", Some("f:c:\\pd"), r"C:\PD\Locked", true),
            entry(
                "f:c:\\pd\\locked\\inside",
                Some("f:c:\\pd\\locked"),
                r"C:\PD\Locked\Inside",
                false,
            ),
            task,
        ]);
        assert!(new_entries(&before, &after).is_empty());
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
        let (main, also) = choose_app(
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
        );
        assert_eq!(main.unwrap().name, "Game");
        let also: Vec<String> = also.into_iter().map(|a| a.name).collect();
        assert_eq!(also, ["Tool", "Microsoft Visual C++ 2015-2022 Redistributable"]);
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
    }

    #[test]
    fn trace_ids_stay_inside_the_traces_folder() {
        let settings = Settings::default();
        assert!(trace_file(&settings, r"..\secrets").is_err());
        assert!(trace_file(&settings, "a.b").is_err());
        assert!(trace_file(&settings, "1700000000-discord").is_ok());
    }
}
