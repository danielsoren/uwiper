//! Quarantine: everything UWiper removes goes into a batch folder first,
//! together with what's needed to put it back. Batches older than the number
//! of days set in Settings are deleted for good.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{blocking, Error, Result};
use crate::paths::data_dir;
use crate::settings::Settings;
use crate::targets::{self, Backup, Guard, Target};
use crate::util::now_secs;
use crate::{log, AppState};

/// One batch is written at a time.
static LOCK: Mutex<()> = Mutex::new(());

const MANIFEST: &str = "manifest.json";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Batch {
    id: String,
    title: String,
    /// Unix time
    created_at: u64,
    items: Vec<Item>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    label: String,
    target: Target,
    backup: Backup,
    size_bytes: u64,
    /// The folder inside the batch that holds the backup.
    dir: String,
    note: Option<String>,
    /// Backed up and recorded, but not removed yet: if UWiper stopped right
    /// here, the next start finds out which way it went.
    #[serde(default)]
    pending: bool,
}

/// Something to remove, as the UI sends it.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Removal {
    label: String,
    target: Target,
    #[serde(default)]
    size_bytes: u64,
    /// What the target looked like when the scan offered it.
    #[serde(default)]
    check: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    label: String,
    error: String,
}

#[derive(Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RemoveReport {
    /// Set when the items went to quarantine.
    batch_id: Option<String>,
    removed: u32,
    freed_bytes: u64,
    problems: Vec<Problem>,
    notes: Vec<String>,
}

#[derive(Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReport {
    restored: u32,
    problems: Vec<Problem>,
    /// Services come back after Windows restarts.
    restart_needed: bool,
}

fn root(settings: &Settings) -> PathBuf {
    data_dir(settings, "quarantine")
}

pub(crate) fn slug(title: &str) -> String {
    let mut out = String::new();
    for ch in title.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
        if out.len() >= 40 {
            break;
        }
    }
    out.trim_end_matches('-').to_string()
}

fn new_id(root: &Path, title: &str) -> String {
    let base = format!("{}-{}", now_secs(), slug(title));
    let mut id = base.clone();
    let mut n = 2;
    while root.join(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// Writes the manifest next to the old one first, so a crash or a full disk
/// never leaves a half-written list behind.
fn save(dir: &Path, batch: &Batch) -> Result<()> {
    let json = serde_json::to_string_pretty(batch).map_err(|e| Error::Other(e.to_string()))?;
    let temp = dir.join(format!("{MANIFEST}.new"));
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(json.as_bytes())?;
        // On disk before anything is removed, not just in a cache.
        file.sync_all()?;
    }
    std::fs::rename(&temp, dir.join(MANIFEST))?;
    Ok(())
}

/// Services, tasks and startup entries go first, so an app's folder isn't
/// still in use by its own service when it's moved; folders go last.
fn removal_order(target: &Target) -> u8 {
    match target {
        Target::Service { .. } | Target::Task { .. } | Target::RegistryValue { .. } | Target::PathEntry { .. } => 0,
        Target::Driver { .. } => 1,
        Target::RegistryKey { .. } => 1,
        Target::Path { .. } => 2,
    }
}

fn load(dir: &Path) -> Option<Batch> {
    serde_json::from_str(&std::fs::read_to_string(dir.join(MANIFEST)).ok()?).ok()
}

/// The batch folder for an id coming from the UI.
fn batch_dir(settings: &Settings, id: &str) -> Result<PathBuf> {
    if id.is_empty() || id.contains(['/', '\\', ':']) || id.starts_with('.') {
        return Err(Error::Other("Unknown quarantine batch".into()));
    }
    let dir = root(settings).join(id);
    if !dir.join(MANIFEST).is_file() {
        return Err(Error::Other("This batch is no longer in quarantine".into()));
    }
    Ok(dir)
}

pub fn remove(settings: &Settings, title: &str, items: Vec<Removal>) -> Result<RemoveReport> {
    let guard = Guard::new(settings);
    remove_checked(settings, title, items, &|target| guard.check(target))
}

type Check<'a> = &'a dyn Fn(&Target) -> std::result::Result<(), String>;

fn remove_checked(settings: &Settings, title: &str, items: Vec<Removal>, check: Check) -> Result<RemoveReport> {
    let _lock = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let keep = settings.safety.quarantine;
    let root = root(settings);
    let id = new_id(&root, title);
    let dir = root.join(&id);
    let mut batch = Batch {
        id: id.clone(),
        title: title.to_string(),
        created_at: now_secs(),
        items: Vec::new(),
    };
    let mut report = RemoveReport::default();
    let mut log = vec![format!("{title}:")];

    let mut items = items;
    items.sort_by_key(|item| removal_order(&item.target));
    let mut stopped: Option<String> = None;
    for (i, item) in items.into_iter().enumerate() {
        if let Some(why) = &stopped {
            report.problems.push(Problem {
                label: item.label,
                error: why.clone(),
            });
            continue;
        }
        if let Err(reason) = check(&item.target) {
            log.push(format!("  skipped {} – {reason}", item.label));
            report.problems.push(Problem {
                label: item.label,
                error: format!("Skipped – {reason}"),
            });
            continue;
        }
        // It may have changed since the scan: an emptied folder filled up, an
        // app installed again, a service fixed.
        if item
            .check
            .as_deref()
            .is_some_and(|c| !targets::unchanged(&item.target, c))
        {
            log.push(format!("  skipped {} – changed since the scan", item.label));
            report.problems.push(Problem {
                label: item.label,
                error: "Changed since the scan – scan again".into(),
            });
            continue;
        }
        let result = if keep {
            let item_dir = format!("{i:03}");
            match quarantine_one(&dir, &item_dir, &item, &mut batch) {
                Ok(note) => Ok(note),
                Err(Failure::Item(e)) => Err(e),
                Err(Failure::Journal(e)) => {
                    let why = format!("Not removed – the quarantine list couldn't be written ({e})");
                    stopped = Some(why.clone());
                    Err(Error::Other(why))
                }
            }
        } else {
            targets::delete(&item.target).map(|_| None)
        };
        match result {
            Ok(note) => {
                log.push(format!("  removed {}", targets::describe(&item.target)));
                if let Some(note) = note {
                    report.notes.push(format!("{}: {note}", item.label));
                }
                report.removed += 1;
                report.freed_bytes += item.size_bytes;
            }
            Err(e) => {
                log.push(format!("  failed {} – {e}", targets::describe(&item.target)));
                report.problems.push(Problem {
                    label: item.label,
                    error: e.to_string(),
                });
            }
        }
    }

    if keep && batch.items.is_empty() {
        let _ = std::fs::remove_dir_all(&dir);
    } else if keep {
        report.batch_id = Some(id);
    }
    log::write(settings, &log);
    Ok(report)
}

enum Failure {
    /// This item couldn't be backed up or removed; the batch goes on.
    Item(Error),
    /// The quarantine list can't be written: nothing more may be removed.
    Journal(Error),
}

/// Backs one item up, records it as pending, removes it and records that.
/// Nothing is removed unless its backup and the record of it are on disk.
fn quarantine_one(
    dir: &Path,
    item_dir: &str,
    item: &Removal,
    batch: &mut Batch,
) -> std::result::Result<Option<String>, Failure> {
    let path = dir.join(item_dir);
    let discard = |batch: &mut Batch| {
        batch.items.retain(|i| i.dir != item_dir);
        let _ = std::fs::remove_dir_all(&path);
    };
    let backup = match targets::back_up(&item.target, &path) {
        Ok(b) => b,
        Err(e) => {
            discard(batch);
            return Err(Failure::Item(e));
        }
    };
    batch.items.push(Item {
        label: item.label.clone(),
        target: item.target.clone(),
        backup: backup.clone(),
        size_bytes: item.size_bytes,
        dir: item_dir.to_string(),
        note: None,
        pending: true,
    });
    if let Err(e) = save(dir, batch) {
        discard(batch);
        return Err(Failure::Journal(e));
    }
    let removed = targets::remove(&item.target, &backup, &path);
    match removed {
        Ok(note) => {
            if let Some(last) = batch.items.iter_mut().find(|i| i.dir == item_dir) {
                last.pending = false;
                last.note = note.clone();
            }
            // Should this fail, the next start sees the item is gone and finishes it.
            save(dir, batch).map_err(Failure::Journal)?;
            Ok(note)
        }
        Err(e) => {
            discard(batch);
            let _ = save(dir, batch);
            Err(Failure::Item(e))
        }
    }
}

/// Finishes what a crash or power cut interrupted: an item recorded as
/// pending is done when its target is gone, and dropped when it's still there.
pub fn recover(settings: &Settings) {
    let Ok(entries) = std::fs::read_dir(root(settings)) else {
        return;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        let Some(mut batch) = load(&dir) else { continue };
        if !batch.items.iter().any(|i| i.pending) {
            continue;
        }
        let mut kept = Vec::new();
        for mut item in batch.items.drain(..) {
            if !item.pending {
                kept.push(item);
                continue;
            }
            let moved_copy = match &item.backup {
                Backup::Moved { file } => std::fs::symlink_metadata(dir.join(&item.dir).join(file)).is_ok(),
                _ => true,
            };
            if moved_copy && !targets::exists(&item.target) {
                item.pending = false;
                kept.push(item);
            } else if moved_copy && matches!(item.backup, Backup::Moved { .. }) {
                // Partly moved: what was moved is kept to put back.
                item.pending = false;
                item.note = Some("Only partly removed – UWiper stopped half-way".into());
                kept.push(item);
            } else {
                let _ = std::fs::remove_dir_all(dir.join(&item.dir));
            }
        }
        batch.items = kept;
        if batch.items.is_empty() {
            let _ = std::fs::remove_dir_all(&dir);
        } else {
            let _ = save(&dir, &batch);
        }
    }
}

pub fn list(settings: &Settings) -> Vec<Batch> {
    let mut batches: Vec<Batch> = std::fs::read_dir(root(settings))
        .map(|entries| entries.flatten().filter_map(|e| load(&e.path())).collect())
        .unwrap_or_default();
    batches.sort_by_key(|b| std::cmp::Reverse(b.created_at));
    batches
}

pub fn restore(settings: &Settings, id: &str) -> Result<RestoreReport> {
    let _lock = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = batch_dir(settings, id)?;
    let mut batch = load(&dir).ok_or(Error::Other("This batch can't be read".into()))?;
    let mut report = RestoreReport::default();
    let mut log = vec![format!("Restore {}:", batch.title)];
    let mut left = Vec::new();
    // Last removed, first restored: a folder comes back before what was inside it.
    for item in batch.items.drain(..).rev() {
        let item_dir = dir.join(&item.dir);
        match targets::restore(&item.target, &item.backup, &item_dir) {
            Ok(()) => {
                log.push(format!("  restored {}", targets::describe(&item.target)));
                report.restored += 1;
                report.restart_needed |= matches!(item.target, Target::Service { .. });
                let _ = std::fs::remove_dir_all(&item_dir);
            }
            Err(e) => {
                log.push(format!("  failed {} – {e}", targets::describe(&item.target)));
                report.problems.push(Problem {
                    label: item.label.clone(),
                    error: e.to_string(),
                });
                left.push(item);
            }
        }
    }
    left.reverse();
    batch.items = left;
    if batch.items.is_empty() {
        let _ = std::fs::remove_dir_all(&dir);
    } else {
        save(&dir, &batch)?;
    }
    log::write(settings, &log);
    Ok(report)
}

pub fn delete(settings: &Settings, id: &str) -> Result<()> {
    let _lock = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = batch_dir(settings, id)?;
    let title = load(&dir).map(|b| b.title).unwrap_or_default();
    std::fs::remove_dir_all(&dir)?;
    log::write(settings, &[format!("Deleted quarantine batch {title} for good")]);
    Ok(())
}

/// Deletes batches past the keep period. Runs at startup.
pub fn purge_expired(settings: &Settings) {
    let max_age = u64::from(settings.safety.keep_days) * 86_400;
    let now = now_secs();
    for batch in list(settings) {
        if now.saturating_sub(batch.created_at) > max_age {
            let _ = delete(settings, &batch.id);
        }
    }
}

pub(crate) fn settings_of(state: &State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub async fn remove_items(state: State<'_, AppState>, title: String, items: Vec<Removal>) -> Result<RemoveReport> {
    let settings = settings_of(&state);
    blocking(move || remove(&settings, &title, items)).await
}

#[tauri::command]
pub async fn list_quarantine(state: State<'_, AppState>) -> Result<Vec<Batch>> {
    let settings = settings_of(&state);
    blocking(move || Ok(list(&settings))).await
}

#[tauri::command]
pub async fn restore_batch(state: State<'_, AppState>, id: String) -> Result<RestoreReport> {
    let settings = settings_of(&state);
    blocking(move || restore(&settings, &id)).await
}

#[tauri::command]
pub async fn delete_batch(state: State<'_, AppState>, id: String) -> Result<()> {
    let settings = settings_of(&state);
    blocking(move || delete(&settings, &id)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_short_ids() {
        assert_eq!(
            slug("Uninstall Discord (x64) – leftovers"),
            "uninstall-discord-x64-leftovers"
        );
        assert_eq!(slug("!!"), "");
    }

    #[test]
    fn quarantines_and_restores_a_batch() {
        let root = std::env::temp_dir().join(format!("uwiper-q-{}", std::process::id()));
        let app = root.join("Vendor").join("App");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(app.join("data.bin"), [1u8; 64]).unwrap();
        let gone = root.join("Vendor").join("Gone");

        let mut settings = Settings::default();
        settings.storage.one_folder = false;
        settings.storage.quarantine = Some(root.join("q").to_string_lossy().into_owned());
        settings.storage.logs = Some(root.join("logs").to_string_lossy().into_owned());
        let items = vec![
            Removal {
                label: "App".into(),
                target: Target::Path {
                    path: app.to_string_lossy().into(),
                },
                size_bytes: 64,
                check: None,
            },
            Removal {
                label: "Gone".into(),
                target: Target::Path {
                    path: gone.to_string_lossy().into(),
                },
                size_bytes: 0,
                check: None,
            },
        ];
        // Temp paths on Linux don't look like Windows paths; the guard has its own tests.
        let report = remove_checked(&settings, "Uninstall App", items, &|_| Ok(())).unwrap();
        assert_eq!(report.removed, 1);
        assert_eq!(report.freed_bytes, 64);
        assert_eq!(report.problems.len(), 1, "the missing folder is reported");
        assert!(report.batch_id.is_some());
        assert!(!app.exists());

        let batches = list(&settings);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].items.len(), 1);

        let restored = restore(&settings, &batches[0].id).unwrap();
        assert_eq!(restored.restored, 1);
        assert!(app.join("data.bin").exists());
        assert!(list(&settings).is_empty(), "an emptied batch is removed");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
