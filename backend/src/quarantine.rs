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
}

/// Something to remove, as the UI sends it.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Removal {
    label: String,
    target: Target,
    #[serde(default)]
    size_bytes: u64,
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

fn slug(title: &str) -> String {
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

fn save(dir: &Path, batch: &Batch) -> Result<()> {
    let json = serde_json::to_string_pretty(batch).map_err(|e| Error::Other(e.to_string()))?;
    std::fs::write(dir.join(MANIFEST), json)?;
    Ok(())
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

    for (i, item) in items.into_iter().enumerate() {
        if let Err(reason) = check(&item.target) {
            log.push(format!("  skipped {} – {reason}", item.label));
            report.problems.push(Problem {
                label: item.label,
                error: format!("Skipped – {reason}"),
            });
            continue;
        }
        let result = if keep {
            let item_dir = format!("{i:03}");
            targets::quarantine(&item.target, &dir.join(&item_dir)).map(|(backup, note)| {
                batch.items.push(Item {
                    label: item.label.clone(),
                    target: item.target.clone(),
                    backup,
                    size_bytes: item.size_bytes,
                    dir: item_dir.clone(),
                    note: note.clone(),
                });
                note
            })
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
                // Keep the manifest current, so a crash still leaves a batch that can be restored.
                if keep {
                    save(&dir, &batch)?;
                }
            }
            Err(e) => {
                if keep {
                    let _ = std::fs::remove_dir_all(dir.join(format!("{i:03}")));
                }
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

fn settings_of(state: &State<'_, AppState>) -> Settings {
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
            },
            Removal {
                label: "Gone".into(),
                target: Target::Path {
                    path: gone.to_string_lossy().into(),
                },
                size_bytes: 0,
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
