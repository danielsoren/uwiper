//! Caches and temporary files that Windows and apps rebuild on their own.
//! Cleaning deletes them for good (no quarantine): apps simply rebuild them.
//!
//! Besides the known locations below, app caches are found by folder name
//! (`Cache`, `GPUCache`, …) in AppData, so apps nobody listed show up too.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::State;

use crate::error::{blocking, Result};
use crate::targets::{self, Guard};
use crate::util::{canonical, expand_env, fresh_time, is_within, parallel_map, pin_dir, pin_parents, Blocked, Pin};
use crate::{log, AppState};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CleanupItem {
    id: String,
    group: &'static str,
    name: String,
    description: String,
    default_on: bool,
    /// Found by folder name rather than from the known list.
    detected: bool,
    size_bytes: u64,
    file_count: u64,
    paths: Vec<String>,
}

enum Spec {
    /// A whole folder.
    Dir(&'static str),
    /// One file.
    File(&'static str),
    /// Files in a folder whose names start with a prefix.
    Prefixed(&'static str, &'static str),
    /// Files in a folder whose names end with a suffix, such as ".log".
    Ending(&'static str, &'static str),
    /// In every subfolder of a folder whose name starts with a prefix, these
    /// folders: `Children("…\\tdata", "user_data", &["cache"])`.
    Children(&'static str, &'static str, &'static [&'static str]),
    /// Cache folders of every profile in a Chromium "User Data" folder.
    Chromium(&'static str),
    /// An app's embedded browser (CEF, WebView2): only its cache folders,
    /// never the profile around them. With a prefix, every folder in the
    /// first one whose name starts with it is such a browser.
    WebView(&'static str, &'static str),
    /// `cache2` of every Firefox profile.
    Firefox(&'static str),
    /// The Recycle Bin on every drive.
    RecycleBin,
}

struct Def {
    id: &'static str,
    group: &'static str,
    name: &'static str,
    description: &'static str,
    default_on: bool,
    specs: &'static [Spec],
}

const BROWSER_NOTE: &str = "Cookies, passwords and history are never touched";
const REDOWNLOAD: &str = "Downloaded again the next time it's needed";
const CHAT_MEDIA: &str = "Images and media from chats, downloaded again when opened";

#[rustfmt::skip]
const DEFS: &[Def] = &[
    Def { id: "temp", group: "windows", name: "Temporary files", description: "%TEMP% and Windows\\Temp – files changed in the last day stay", default_on: true,
        specs: &[Spec::Dir("%TEMP%"), Spec::Dir(r"%SystemRoot%\Temp")] },
    Def { id: "windows-update", group: "windows", name: "Windows Update downloads", description: "Update packages Windows downloaded – only clean them when no update is waiting to install", default_on: false,
        specs: &[Spec::Dir(r"%SystemRoot%\SoftwareDistribution\Download")] },
    Def { id: "delivery-optimization", group: "windows", name: "Delivery Optimization", description: "Update files shared with other PCs", default_on: true,
        specs: &[Spec::Dir(r"%SystemRoot%\ServiceProfiles\NetworkService\AppData\Local\Microsoft\Windows\DeliveryOptimization\Cache")] },
    Def { id: "crash-dumps", group: "windows", name: "Crash reports & memory dumps", description: "Kept to find out why something crashed – nothing rebuilds them", default_on: false,
        specs: &[
            Spec::Dir(r"%ProgramData%\Microsoft\Windows\WER\ReportArchive"),
            Spec::Dir(r"%ProgramData%\Microsoft\Windows\WER\ReportQueue"),
            Spec::Dir(r"%LOCALAPPDATA%\CrashDumps"),
            Spec::Dir(r"%SystemRoot%\Minidump"),
            Spec::File(r"%SystemRoot%\MEMORY.DMP"),
        ] },
    Def { id: "windows-logs", group: "windows", name: "Windows log files", description: "Servicing and setup logs (CBS, DISM)", default_on: true,
        specs: &[
            Spec::Ending(r"%SystemRoot%\Logs\CBS", ".log"),
            Spec::Ending(r"%SystemRoot%\Logs\CBS", ".cab"),
            Spec::Ending(r"%SystemRoot%\Logs\DISM", ".log"),
        ] },
    Def { id: "shader-cache", group: "windows", name: "Shader caches", description: "DirectX and GPU driver caches, rebuilt by games", default_on: true,
        specs: &[
            Spec::Dir(r"%LOCALAPPDATA%\D3DSCache"),
            Spec::Dir(r"%LOCALAPPDATA%\NVIDIA\DXCache"),
            Spec::Dir(r"%LOCALAPPDATA%\NVIDIA\GLCache"),
            Spec::Dir(r"%LOCALAPPDATA%\AMD\DxCache"),
            Spec::Dir(r"%ProgramData%\NVIDIA Corporation\NV_Cache"),
        ] },
    Def { id: "inetcache", group: "windows", name: "Internet cache", description: "Web content cached by Windows and older apps", default_on: true,
        specs: &[Spec::Dir(r"%LOCALAPPDATA%\Microsoft\Windows\INetCache")] },
    Def { id: "thumbnails", group: "windows", name: "Thumbnail cache", description: "Explorer previews, rebuilt on demand", default_on: true,
        specs: &[Spec::Prefixed(r"%LOCALAPPDATA%\Microsoft\Windows\Explorer", "thumbcache_")] },
    Def { id: "recycle-bin", group: "windows", name: "Recycle Bin", description: "Files you deleted – emptying it can't be undone", default_on: false,
        specs: &[Spec::RecycleBin] },

    Def { id: "chrome", group: "browsers", name: "Google Chrome – cache", description: BROWSER_NOTE, default_on: true,
        specs: &[Spec::Chromium(r"%LOCALAPPDATA%\Google\Chrome\User Data")] },
    Def { id: "edge", group: "browsers", name: "Microsoft Edge – cache", description: BROWSER_NOTE, default_on: true,
        specs: &[Spec::Chromium(r"%LOCALAPPDATA%\Microsoft\Edge\User Data")] },
    Def { id: "brave", group: "browsers", name: "Brave – cache", description: BROWSER_NOTE, default_on: true,
        specs: &[Spec::Chromium(r"%LOCALAPPDATA%\BraveSoftware\Brave-Browser\User Data")] },
    Def { id: "yandex", group: "browsers", name: "Yandex Browser – cache", description: BROWSER_NOTE, default_on: true,
        specs: &[Spec::Chromium(r"%LOCALAPPDATA%\Yandex\YandexBrowser\User Data")] },
    Def { id: "vivaldi", group: "browsers", name: "Vivaldi – cache", description: BROWSER_NOTE, default_on: true,
        specs: &[Spec::Chromium(r"%LOCALAPPDATA%\Vivaldi\User Data")] },
    Def { id: "chromium", group: "browsers", name: "Chromium – cache", description: BROWSER_NOTE, default_on: true,
        specs: &[Spec::Chromium(r"%LOCALAPPDATA%\Chromium\User Data")] },
    Def { id: "opera", group: "browsers", name: "Opera – cache", description: BROWSER_NOTE, default_on: true,
        specs: &[
            Spec::Dir(r"%LOCALAPPDATA%\Opera Software\Opera Stable\Cache"),
            Spec::Dir(r"%APPDATA%\Opera Software\Opera Stable\Code Cache"),
            Spec::Dir(r"%APPDATA%\Opera Software\Opera Stable\GPUCache"),
            Spec::Dir(r"%LOCALAPPDATA%\Opera Software\Opera GX Stable\Cache"),
            Spec::Dir(r"%APPDATA%\Opera Software\Opera GX Stable\Code Cache"),
            Spec::Dir(r"%APPDATA%\Opera Software\Opera GX Stable\GPUCache"),
        ] },
    Def { id: "firefox", group: "browsers", name: "Firefox – cache", description: BROWSER_NOTE, default_on: true,
        specs: &[Spec::Firefox(r"%LOCALAPPDATA%\Mozilla\Firefox\Profiles")] },

    Def { id: "npm", group: "developer", name: "npm cache", description: REDOWNLOAD, default_on: false,
        specs: &[Spec::Dir(r"%LOCALAPPDATA%\npm-cache")] },
    Def { id: "pnpm", group: "developer", name: "pnpm cache", description: "Metadata cache; the package store is left alone", default_on: false,
        specs: &[Spec::Dir(r"%LOCALAPPDATA%\pnpm-cache")] },
    Def { id: "yarn", group: "developer", name: "Yarn cache", description: REDOWNLOAD, default_on: false,
        specs: &[Spec::Dir(r"%LOCALAPPDATA%\Yarn\Cache")] },
    Def { id: "bun", group: "developer", name: "Bun cache", description: REDOWNLOAD, default_on: false,
        specs: &[Spec::Dir(r"%USERPROFILE%\.bun\install\cache")] },
    Def { id: "pip", group: "developer", name: "pip cache", description: REDOWNLOAD, default_on: false,
        specs: &[Spec::Dir(r"%LOCALAPPDATA%\pip\Cache")] },
    Def { id: "nuget", group: "developer", name: "NuGet caches", description: "HTTP and v3 caches; the global packages folder is left alone", default_on: false,
        specs: &[
            Spec::Dir(r"%LOCALAPPDATA%\NuGet\v3-cache"),
            Spec::Dir(r"%LOCALAPPDATA%\NuGet\http-cache"),
            Spec::Dir(r"%LOCALAPPDATA%\NuGet\plugins-cache"),
        ] },
    Def { id: "cargo", group: "developer", name: "Cargo registry cache", description: "Downloaded .crate archives", default_on: false,
        specs: &[Spec::Dir(r"%USERPROFILE%\.cargo\registry\cache")] },
    Def { id: "go", group: "developer", name: "Go build cache", description: "Rebuilt on the next build", default_on: false,
        specs: &[Spec::Dir(r"%LOCALAPPDATA%\go-build")] },
    Def { id: "gradle", group: "developer", name: "Gradle caches", description: REDOWNLOAD, default_on: false,
        specs: &[Spec::Dir(r"%USERPROFILE%\.gradle\caches")] },
    Def { id: "maven", group: "developer", name: "Maven repository", description: REDOWNLOAD, default_on: false,
        specs: &[Spec::Dir(r"%USERPROFILE%\.m2\repository")] },
    Def { id: "electron", group: "developer", name: "Electron downloads", description: "Electron and electron-builder caches", default_on: false,
        specs: &[
            Spec::Dir(r"%LOCALAPPDATA%\electron\Cache"),
            Spec::Dir(r"%LOCALAPPDATA%\electron-builder\Cache"),
            Spec::Dir(r"%LOCALAPPDATA%\node-gyp\Cache"),
        ] },
    Def { id: "unreal", group: "developer", name: "Unreal Engine – shared cache", description: "Derived data, rebuilt when projects open", default_on: false,
        specs: &[Spec::Dir(r"%LOCALAPPDATA%\UnrealEngine\Common\DerivedDataCache")] },
    Def { id: "jetbrains", group: "developer", name: "JetBrains IDE caches", description: "Indexes are rebuilt when a project opens", default_on: false,
        specs: &[Spec::Children(r"%LOCALAPPDATA%\JetBrains", "", &["caches"])] },

    Def { id: "telegram", group: "apps", name: "Telegram – cache", description: CHAT_MEDIA, default_on: true,
        specs: &[Spec::Children(r"%APPDATA%\Telegram Desktop\tdata", "user_data", &["cache", "media_cache"])] },
    Def { id: "discord", group: "apps", name: "Discord – cache", description: CHAT_MEDIA, default_on: true,
        specs: &[Spec::Dir(r"%APPDATA%\discord\Cache"), Spec::Dir(r"%APPDATA%\discord\Code Cache"), Spec::Dir(r"%APPDATA%\discord\GPUCache")] },
    Def { id: "slack", group: "apps", name: "Slack – cache", description: CHAT_MEDIA, default_on: true,
        specs: &[Spec::Dir(r"%APPDATA%\Slack\Cache"), Spec::Dir(r"%APPDATA%\Slack\Code Cache"), Spec::Dir(r"%APPDATA%\Slack\GPUCache")] },
    Def { id: "teams", group: "apps", name: "Microsoft Teams (classic) – cache", description: CHAT_MEDIA, default_on: true,
        specs: &[Spec::Dir(r"%APPDATA%\Microsoft\Teams\Cache"), Spec::Dir(r"%APPDATA%\Microsoft\Teams\Code Cache"), Spec::Dir(r"%APPDATA%\Microsoft\Teams\GPUCache")] },
    Def { id: "vscode", group: "apps", name: "VS Code – cache", description: "Rebuilt when VS Code starts", default_on: true,
        specs: &[
            Spec::Dir(r"%APPDATA%\Code\Cache"),
            Spec::Dir(r"%APPDATA%\Code\CachedData"),
            Spec::Dir(r"%APPDATA%\Code\Code Cache"),
            Spec::Dir(r"%APPDATA%\Code\GPUCache"),
        ] },
    Def { id: "steam", group: "apps", name: "Steam – web cache", description: "Store and community pages; sign-in, cookies and games are kept", default_on: true,
        specs: &[Spec::WebView(r"%LOCALAPPDATA%\Steam\htmlcache", "")] },
    Def { id: "epic", group: "apps", name: "Epic Games Launcher – web cache", description: "Store pages; sign-in, cookies and games are kept", default_on: true,
        specs: &[Spec::WebView(r"%LOCALAPPDATA%\EpicGamesLauncher\Saved", "webcache")] },
    Def { id: "battlenet", group: "apps", name: "Battle.net – cache", description: "Launcher and store pages; sign-in and games are kept", default_on: true,
        specs: &[Spec::WebView(r"%LOCALAPPDATA%\Battle.net\BrowserCaches", ""), Spec::Dir(r"%LOCALAPPDATA%\Battle.net\Cache")] },
    Def { id: "adobe-media", group: "apps", name: "Adobe media cache", description: "Premiere Pro and After Effects rebuild it when a project opens", default_on: false,
        specs: &[
            Spec::Dir(r"%APPDATA%\Adobe\Common\Media Cache Files"),
            Spec::Dir(r"%APPDATA%\Adobe\Common\Media Cache"),
            Spec::Dir(r"%APPDATA%\Adobe\Common\Peak Files"),
        ] },
    Def { id: "spotify", group: "apps", name: "Spotify – cache", description: "Streaming cache; offline downloads are kept", default_on: false,
        specs: &[Spec::Dir(r"%LOCALAPPDATA%\Spotify\Data")] },
];

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|it| it.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default()
}

fn entries_where(dir: &str, dirs: bool, keep: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    std::fs::read_dir(expand_env(dir))
        .map(|it| {
            it.flatten()
                .filter(|e| e.path().is_dir() == dirs && keep(&e.file_name().to_string_lossy().to_lowercase()))
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default()
}

fn resolve(spec: &Spec) -> Vec<PathBuf> {
    let existing = |p: PathBuf| p.exists().then_some(p);
    match spec {
        Spec::Dir(p) | Spec::File(p) => existing(PathBuf::from(expand_env(p))).into_iter().collect(),
        Spec::Prefixed(dir, prefix) => entries_where(dir, false, |name| name.starts_with(&prefix.to_lowercase())),
        Spec::Ending(dir, suffix) => entries_where(dir, false, |name| name.ends_with(&suffix.to_lowercase())),
        Spec::Children(root, prefix, subs) => subdirs(Path::new(&expand_env(root)))
            .into_iter()
            .filter(|child| {
                child
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().to_lowercase().starts_with(prefix))
            })
            .flat_map(|child| {
                subs.iter()
                    .filter_map(move |sub| existing(child.join(sub)))
                    .collect::<Vec<_>>()
            })
            .collect(),
        Spec::Chromium(user_data) => chromium_caches(Path::new(&expand_env(user_data))),
        Spec::WebView(dir, "") => webview_caches(Path::new(&expand_env(dir))),
        Spec::WebView(dir, prefix) => entries_where(dir, true, |name| name.starts_with(&prefix.to_lowercase()))
            .iter()
            .flat_map(|d| webview_caches(d))
            .collect(),
        Spec::Firefox(profiles) => subdirs(Path::new(&expand_env(profiles)))
            .into_iter()
            .filter_map(|p| existing(p.join("cache2")))
            .collect(),
        Spec::RecycleBin => Vec::new(),
    }
}

/// What cleaning leaves alone, so measuring counts only what goes.
struct Rules<'a> {
    own: &'a [String],
    /// Files changed after this stay (Temp: what's in use today).
    keep_newer_than: Option<std::time::SystemTime>,
}

impl Rules<'_> {
    fn for_category<'a>(id: &str, own: &'a [String]) -> Rules<'a> {
        Rules {
            own,
            keep_newer_than: (id == "temp")
                .then(|| std::time::SystemTime::now() - std::time::Duration::from_secs(KEEP_RECENT_TEMP_SECS)),
        }
    }

    /// Links, profile data, Office's opened attachments and UWiper's own folders.
    fn skips(&self, path: &Path, meta: &std::fs::Metadata) -> bool {
        let name = name_of(path).to_lowercase();
        is_link(meta)
            || PROFILE_DATA.contains(&name.as_str())
            || ["content.outlook", "content.mso"].contains(&name.as_str())
            || is_own(path, self.own)
    }

    /// Made or written after the cutoff: an installer unpacking right now,
    /// or a running program's unpacked files, keep their old write times.
    fn too_new(&self, meta: &std::fs::Metadata) -> bool {
        self.keep_newer_than
            .is_some_and(|cutoff| fresh_time(meta).is_none_or(|t| t > cutoff))
    }
}

/// Temporary files changed within the last day may belong to something running.
const KEEP_RECENT_TEMP_SECS: u64 = 24 * 3600;

/// A link or junction (any reparse point): it may lead anywhere.
fn is_link(meta: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    meta.file_type().is_symlink()
}

/// Pins the folders on the way to a path to clean, so none of them can be
/// swapped for a link while it's cleaned. Links at or above the user's and
/// Windows' own folders are fine; below them, cleaning could reach anywhere.
fn reach(path: &Path) -> std::result::Result<Vec<Pin>, Blocked> {
    if std::fs::symlink_metadata(path).is_ok_and(|m| is_link(&m)) {
        return Err(Blocked::Link);
    }
    pin_parents(path)
}

/// A folder to clean that holds the user's own files: a drive root, the
/// profile or one of its folders, or anything holding one – where %TEMP%
/// points when it's set wrong. Cleaning can't be undone, so it's refused.
fn holds_user_files(path: &Path) -> bool {
    let p = canonical(&path.to_string_lossy());
    path.parent().is_none()
        || p.trim_end_matches('\\').len() <= 2
        || targets::shared_folders(false)
            .iter()
            .any(|f| is_within(&canonical(f), &p))
}

fn measure_dir(dir: &Path, rules: &Rules, total: &mut (u64, u64)) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if rules.skips(&path, &meta) {
            continue;
        }
        if meta.is_dir() {
            measure_dir(&path, rules, total);
        } else if !rules.too_new(&meta) {
            total.0 += meta.len();
            total.1 += 1;
        }
    }
}

fn measure_paths(paths: &[PathBuf], rules: &Rules) -> (u64, u64) {
    let mut total = (0, 0);
    for p in paths {
        let Ok(meta) = std::fs::symlink_metadata(p) else {
            continue;
        };
        if holds_user_files(p) || reach(p).is_err() || rules.skips(p, &meta) {
            continue;
        }
        if meta.is_dir() {
            measure_dir(p, rules, &mut total);
        } else if !rules.too_new(&meta) {
            total.0 += meta.len();
            total.1 += 1;
        }
    }
    total
}

fn measure(def: &Def) -> Option<CleanupItem> {
    let item = |size_bytes, file_count, paths: Vec<PathBuf>| CleanupItem {
        id: def.id.into(),
        group: def.group,
        name: def.name.into(),
        description: def.description.into(),
        default_on: def.default_on,
        detected: false,
        size_bytes,
        file_count,
        paths: paths.iter().map(|p| p.to_string_lossy().into_owned()).collect(),
    };
    if def.specs.iter().any(|s| matches!(s, Spec::RecycleBin)) {
        let (bytes, count) = imp::recycle_bin();
        return (count > 0).then(|| item(bytes, count, Vec::new()));
    }
    let paths: Vec<PathBuf> = def.specs.iter().flat_map(resolve).collect();
    if paths.is_empty() {
        return None;
    }
    let (size_bytes, file_count) = measure_paths(&paths, &Rules::for_category(def.id, &[]));
    Some(item(size_bytes, file_count, paths))
}

// ---------------------------------------------------------------------------
// App caches found by folder name

/// Chromium's own cache folders (every Electron app has them): safe to
/// clean by default.
const CHROMIUM_CACHES: [&str; 8] = [
    "code cache",
    "gpucache",
    "dawncache",
    "dawngraphitecache",
    "dawnwebgpucache",
    "graphitedawncache",
    "grshadercache",
    "shadercache",
];

/// Folders a cache search never walks into: they hold code or environments.
const NEVER_INSIDE: [&str; 6] = ["site-packages", "node_modules", "virtualenvs", ".venv", "venv", "lib"];

/// What browsers and embedded browsers keep in a profile: sign-ins, cookies,
/// history, site data, settings. Never cleaned, whichever category a folder
/// comes from.
const PROFILE_DATA: &[&str] = &[
    "cookies",
    "cookies-journal",
    "network",
    "history",
    "history-journal",
    "login data",
    "login data-journal",
    "login data for account",
    "web data",
    "web data-journal",
    "local storage",
    "session storage",
    "indexeddb",
    "databases",
    "service worker",
    "preferences",
    "secure preferences",
    "local state",
    "bookmarks",
    "favicons",
    "top sites",
    "extensions",
    "extension state",
    "local extension settings",
    "sync extension settings",
    "managed extension settings",
    "shared_proto_db",
    "sessions",
    "current session",
    "current tabs",
    "last session",
    "last tabs",
    "visited links",
    "storage",
    "file system",
    "webstorage",
    "transportsecurity",
    "network persistent state",
    "trust tokens",
    "sync data",
];

fn holds_profile_data(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries
            .flatten()
            .any(|e| PROFILE_DATA.contains(&e.file_name().to_string_lossy().to_lowercase().as_str()))
    })
}

/// A Chromium cache folder: one of the names above, or a "Cache" with
/// Chromium's layout inside.
fn is_chromium_cache(dir: &Path) -> bool {
    let name = name_of(dir).to_lowercase();
    CHROMIUM_CACHES.contains(&name.as_str())
        || (name == "cache" && (dir.join("Cache_Data").is_dir() || dir.join("index").is_file()))
}

/// Folder names apps use for caches they rebuild.
const CACHE_NAMES: [&str; 12] = [
    "cache",
    "caches",
    "code cache",
    "gpucache",
    "dawncache",
    "dawngraphitecache",
    "dawnwebgpucache",
    "graphitedawncache",
    "grshadercache",
    "shadercache",
    "cacheddata",
    "media_cache",
];

/// Folders in AppData that aren't one app's: Windows' own, packaged apps,
/// installed programs, and the tool caches listed above.
const NOT_APPS: &[&str] = &[
    "microsoft",
    "packages",
    "temp",
    "programs",
    "comms",
    "connecteddevicesplatform",
    "placeholdertilelogofolder",
    "publishers",
    "virtualstore",
    "crashdumps",
    "d3dscache",
    "nvidia",
    "amd",
    "intel",
    "pypoetry",
];

struct Detected {
    id: String,
    group: &'static str,
    name: String,
    description: String,
    paths: Vec<PathBuf>,
    default_on: bool,
    /// Store apps' tiny caches aren't worth a row.
    min_bytes: u64,
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Browsers found by their layout rather than by name: any Chromium browser
/// (a "User Data" folder with profiles) and any Firefox-based one
/// ("Profiles" with `cache2` inside), wherever its vendor put it.
fn detect_browsers(known: &[String]) -> Vec<Detected> {
    let mut out = Vec::new();
    for root in ["%LOCALAPPDATA%", "%APPDATA%"]
        .iter()
        .map(|r| expand_env(r))
        .filter(|r| !r.contains('%'))
    {
        let walker = walkdir::WalkDir::new(&root)
            .min_depth(1)
            .max_depth(3)
            .follow_links(false);
        for entry in walker
            .into_iter()
            .filter_entry(|e| !NOT_APPS.contains(&name_of(e.path()).to_lowercase().as_str()))
        {
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_dir() {
                continue;
            }
            let dir = entry.path();
            let kind = match name_of(dir).to_lowercase().as_str() {
                "user data" => Spec::Chromium(""),
                "profiles" => Spec::Firefox(""),
                _ => continue,
            };
            // Only real browser profiles: Chromium keeps "Local State" next to
            // them, Firefox an "entries" folder in each cache2.
            let paths: Vec<PathBuf> = match kind {
                Spec::Chromium(_) if dir.join("Local State").is_file() => chromium_caches(dir),
                Spec::Chromium(_) => Vec::new(),
                _ => subdirs(dir)
                    .into_iter()
                    .map(|p| p.join("cache2"))
                    .filter(|p| p.join("entries").is_dir())
                    .collect(),
            };
            let canon = canonical(&dir.to_string_lossy());
            let covered = paths.iter().any(|p| known.contains(&canonical(&p.to_string_lossy())));
            if paths.is_empty() || covered {
                continue;
            }
            let product = dir.parent().map(name_of).unwrap_or_default();
            out.push(Detected {
                id: format!("found:{canon}"),
                group: "browsers",
                name: format!("{product} – cache"),
                description: format!("{BROWSER_NOTE} · found by its layout"),
                paths,
                default_on: true,
                min_bytes: 0,
            });
        }
    }
    out
}

fn chromium_caches(user_data: &Path) -> Vec<PathBuf> {
    let existing = |p: PathBuf| p.is_dir().then_some(p);
    let mut out: Vec<PathBuf> = ["GrShaderCache", "ShaderCache"]
        .iter()
        .filter_map(|d| existing(user_data.join(d)))
        .collect();
    for profile in subdirs(user_data) {
        for d in ["Cache", "Code Cache", "GPUCache"] {
            out.extend(existing(profile.join(d)));
        }
    }
    out
}

/// The cache folders of an embedded browser: next to its profiles and in
/// each of them, up to two levels down.
fn webview_caches(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut walker = walkdir::WalkDir::new(root)
        .min_depth(1)
        .max_depth(3)
        .follow_links(false)
        .into_iter();
    while let Some(Ok(entry)) = walker.next() {
        if !entry.file_type().is_dir() {
            continue;
        }
        let path = entry.path();
        // A "Cache" that holds a profile's data is a profile, not a cache.
        if (is_chromium_cache(path) || name_of(path).eq_ignore_ascii_case("cache")) && !holds_profile_data(path) {
            out.push(path.to_path_buf());
            walker.skip_current_dir();
        } else if PROFILE_DATA.contains(&name_of(path).to_lowercase().as_str()) {
            walker.skip_current_dir();
        }
    }
    out
}

/// Cache folders, by name, up to `depth` levels below `dir`.
fn cache_folders(dir: &Path, depth: usize) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut walker = walkdir::WalkDir::new(dir)
        .min_depth(1)
        .max_depth(depth)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !NEVER_INSIDE.contains(&name_of(e.path()).to_lowercase().as_str()));
    while let Some(Ok(entry)) = walker.next() {
        let path = entry.path();
        if entry.file_type().is_dir() && CACHE_NAMES.contains(&name_of(path).to_lowercase().as_str()) {
            // A browser profile named "Cache" holds sign-ins and cookies: only
            // the caches inside it count, found further down.
            if holds_profile_data(path) {
                continue;
            }
            // Poetry and others keep environments inside "cache".
            if !NEVER_INSIDE.iter().any(|n| path.join(n).exists()) {
                paths.push(path.to_path_buf());
            }
            walker.skip_current_dir();
        }
    }
    paths
}

fn folder_names(paths: &[PathBuf]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for name in paths.iter().map(|p| name_of(p)) {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// App folders in AppData with cache folders inside (up to three levels
/// down), skipping apps the known list already covers.
fn detect_apps(known: &[String], own: &[String]) -> Vec<Detected> {
    let roots = ["%APPDATA%", "%LOCALAPPDATA%", r"%USERPROFILE%\AppData\LocalLow"];
    let mut out = Vec::new();
    for root in roots.iter().map(|r| expand_env(r)).filter(|r| !r.contains('%')) {
        for app in subdirs(Path::new(&root)) {
            let app_name = name_of(&app);
            let app_canon = canonical(&app.to_string_lossy());
            let covered = known.iter().any(|k| is_within(k, &app_canon));
            let ours = own.iter().any(|o| is_within(o, &app_canon) || is_within(&app_canon, o));
            if NOT_APPS.contains(&app_name.to_lowercase().as_str()) || app_name.starts_with('.') || covered || ours {
                continue;
            }
            let (chromium, other): (Vec<PathBuf>, Vec<PathBuf>) =
                cache_folders(&app, 3).into_iter().partition(|p| is_chromium_cache(p));
            for (paths, on, suffix) in [(chromium, true, ""), (other, false, ":other")] {
                if paths.is_empty() {
                    continue;
                }
                let names = folder_names(&paths);
                out.push(Detected {
                    id: format!("found:{app_canon}{suffix}"),
                    group: "apps",
                    name: if on {
                        format!("{app_name} – cache")
                    } else {
                        format!("{app_name} – other caches")
                    },
                    description: format!("Found by folder name: {}", names.join(", ")),
                    default_on: on,
                    paths,
                    min_bytes: 0,
                });
            }
        }
    }
    out
}

/// "5319275A.WhatsAppDesktop_cv1g1gvanyjgm" → "WhatsAppDesktop",
/// "PythonSoftwareFoundation.Python.3.12_qbz…" → "Python"
fn package_name(family: &str) -> String {
    let base = family.split('_').next().unwrap_or(family);
    let words: Vec<&str> = base
        .split('.')
        .skip(1)
        .filter(|w| !w.is_empty() && !w.chars().all(|c| c.is_ascii_digit()))
        .collect();
    if words.is_empty() {
        base.to_string()
    } else {
        words.join(" ")
    }
}

/// Microsoft Store apps keep their caches in their package folder: the
/// web cache and temp folders Windows gives every app, plus cache folders
/// by name in LocalCache. LocalCache itself can hold real data, so it is
/// never emptied as a whole.
fn detect_store_apps(own: &[String]) -> Vec<Detected> {
    let packages = PathBuf::from(expand_env(r"%LOCALAPPDATA%\Packages"));
    let mut out = Vec::new();
    for pkg in subdirs(&packages) {
        let pkg_canon = canonical(&pkg.to_string_lossy());
        if own.iter().any(|o| is_within(o, &pkg_canon)) {
            continue;
        }
        let mut paths: Vec<PathBuf> = [r"AC\INetCache", r"AC\Temp", "TempState"]
            .iter()
            .map(|d| pkg.join(d))
            .filter(|p| p.is_dir())
            .collect();
        // In LocalCache only Chromium's caches (an app's web view): other
        // folders named "cache" there can be libraries or data.
        paths.extend(
            cache_folders(&pkg.join("LocalCache"), 5)
                .into_iter()
                .filter(|p| is_chromium_cache(p)),
        );
        if paths.is_empty() {
            continue;
        }
        out.push(Detected {
            id: format!("found:{pkg_canon}"),
            group: "apps",
            name: format!("{} (Store app) – cache", package_name(&name_of(&pkg))),
            description: "Web cache and temporary files of a Microsoft Store app".into(),
            default_on: true,
            paths,
            min_bytes: 1024 * 1024,
        });
    }
    out
}

/// Everything found beyond the known list.
fn detect(known: &[String], own: &[String]) -> Vec<Detected> {
    let browsers = detect_browsers(known);
    let mut known = known.to_vec();
    known.extend(
        browsers
            .iter()
            .flat_map(|b| b.paths.iter().map(|p| canonical(&p.to_string_lossy()))),
    );
    let mut out = browsers;
    out.extend(detect_apps(&known, own));
    out.extend(detect_store_apps(own));
    out
}

fn known_paths() -> Vec<String> {
    DEFS.iter()
        .flat_map(|d| d.specs.iter().flat_map(resolve))
        .map(|p| canonical(&p.to_string_lossy()))
        .collect()
}

#[tauri::command]
pub async fn scan_cleanup(state: State<'_, AppState>) -> Result<Vec<CleanupItem>> {
    let settings = state.settings.lock().unwrap().clone();
    blocking(move || {
        // Each category walks its own folders, so measure them in parallel.
        let mut items: Vec<CleanupItem> = std::thread::scope(|scope| {
            let handles: Vec<_> = DEFS.iter().map(|def| scope.spawn(move || measure(def))).collect();
            handles.into_iter().filter_map(|h| h.join().ok().flatten()).collect()
        });
        let guard = Guard::new(&settings);
        let mut detected = detect(&known_paths(), &guard.own);
        for d in &mut detected {
            d.paths.retain(|p| !guard.is_excluded(&p.to_string_lossy()));
        }
        detected.retain(|d| !d.paths.is_empty());
        let sizes = parallel_map(&detected, |d| {
            measure_paths(&d.paths, &Rules::for_category(&d.id, &guard.own))
        });
        for (d, (size_bytes, file_count)) in detected.into_iter().zip(sizes) {
            if size_bytes < d.min_bytes {
                continue;
            }
            items.push(CleanupItem {
                id: d.id,
                group: d.group,
                name: d.name,
                description: d.description,
                default_on: d.default_on,
                detected: true,
                size_bytes,
                file_count,
                paths: d.paths.iter().map(|p| p.to_string_lossy().into_owned()).collect(),
            });
        }
        Ok(items)
    })
    .await
}

#[derive(Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CleanReport {
    freed_bytes: u64,
    /// Files that were in use, typically by a running browser.
    skipped_files: u64,
    /// Files Windows wouldn't let UWiper delete (no access, read-only).
    denied_files: u64,
    /// Files that couldn't be deleted for another reason.
    failed_files: u64,
    /// Folders left alone because they lead through a link or junction.
    skipped_links: u64,
    /// Folders UWiper couldn't open.
    unreadable_folders: u64,
    /// Folders to clean that hold the user's own files (%TEMP% set to the
    /// profile, say), left alone.
    refused: Vec<String>,
}

/// Deletes the contents of the chosen cache categories. Folders themselves
/// stay; links, profile data and files Windows won't let go of are skipped.
#[tauri::command]
pub async fn clean_caches(state: State<'_, AppState>, ids: Vec<String>) -> Result<CleanReport> {
    let settings = state.settings.lock().unwrap().clone();
    blocking(move || {
        let mut report = CleanReport::default();
        let mut lines = vec!["Cleaned caches:".to_string()];
        // UWiper may run from inside %TEMP% (an opened zip) or keep its data there.
        let guard = Guard::new(&settings);
        let own = guard.own.clone();
        let mut jobs: Vec<(String, &str, Vec<PathBuf>)> = Vec::new();
        for def in DEFS.iter().filter(|d| ids.iter().any(|id| id == d.id)) {
            if def.specs.iter().any(|s| matches!(s, Spec::RecycleBin)) {
                let (bytes, _) = imp::recycle_bin();
                if imp::empty_recycle_bin() {
                    report.freed_bytes += bytes;
                    lines.push(format!("  {} – {bytes} bytes", def.name));
                }
                continue;
            }
            jobs.push((
                def.name.to_string(),
                def.id,
                def.specs.iter().flat_map(resolve).collect(),
            ));
        }
        let found: Vec<Detected> = if ids.iter().any(|id| id.starts_with("found:")) {
            detect(&known_paths(), &own)
                .into_iter()
                .filter(|d| ids.contains(&d.id))
                .collect()
        } else {
            Vec::new()
        };
        for d in &found {
            jobs.push((d.name.clone(), d.id.as_str(), d.paths.clone()));
        }
        for (name, id, paths) in jobs {
            let before = report.freed_bytes;
            let rules = Rules::for_category(id, &own);
            for path in paths {
                if guard.is_excluded(&path.to_string_lossy()) {
                    continue;
                }
                if holds_user_files(&path) {
                    lines.push(format!("  left alone {} – it holds your own files", path.display()));
                    report.refused.push(path.to_string_lossy().into_owned());
                    continue;
                }
                // Held until it's cleaned.
                let _pins = match reach(&path) {
                    Ok(pins) => pins,
                    Err(Blocked::Link) => {
                        report.skipped_links += 1;
                        continue;
                    }
                    // Nothing to clean, or nothing the scan could measure either.
                    Err(Blocked::Gone | Blocked::Unreadable) => continue,
                };
                let Ok(meta) = std::fs::symlink_metadata(&path) else {
                    continue;
                };
                if rules.skips(&path, &meta) {
                    continue;
                }
                if meta.is_dir() {
                    empty_dir(&path, &rules, &mut report, true);
                } else if !rules.too_new(&meta) {
                    remove_file(&path, &meta, &mut report);
                }
            }
            lines.push(format!("  {name} – {} bytes", report.freed_bytes - before));
        }
        if report.skipped_links > 0 {
            lines.push(format!(
                "  {} folders skipped: they lead through a link",
                report.skipped_links
            ));
        }
        if report.unreadable_folders > 0 {
            lines.push(format!("  {} folders couldn't be opened", report.unreadable_folders));
        }
        log::write(&settings, &lines);
        Ok(report)
    })
    .await
}

fn remove_file(path: &Path, meta: &std::fs::Metadata, report: &mut CleanReport) {
    let mut result = std::fs::remove_file(path);
    // Read-only files go once the flag is cleared; it's put back if they still don't.
    let denied = |r: &std::io::Result<()>| {
        r.as_ref()
            .is_err_and(|e| e.kind() == std::io::ErrorKind::PermissionDenied)
    };
    if denied(&result) && meta.permissions().readonly() {
        let mut writable = meta.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        writable.set_readonly(false);
        if std::fs::set_permissions(path, writable).is_ok() {
            result = std::fs::remove_file(path);
            if result.is_err() {
                let _ = std::fs::set_permissions(path, meta.permissions());
            }
        }
    }
    let sharing = |e: &std::io::Error| matches!(e.raw_os_error(), Some(32 | 33));
    match result {
        Ok(()) => report.freed_bytes += meta.len(),
        // Sharing and lock violations: an app has it open.
        Err(e) if sharing(&e) => report.skipped_files += 1,
        // A running program can't be deleted either, but Windows calls that
        // "access denied"; it can't be opened for writing then.
        Err(e)
            if e.kind() == std::io::ErrorKind::PermissionDenied
                && std::fs::OpenOptions::new()
                    .write(true)
                    .open(path)
                    .is_err_and(|e| sharing(&e)) =>
        {
            report.skipped_files += 1
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => report.denied_files += 1,
        Err(_) => report.failed_files += 1,
    }
}

/// True for UWiper's own folders and anything in them.
fn is_own(path: &Path, own: &[String]) -> bool {
    let p = canonical(&path.to_string_lossy());
    own.iter().any(|dir| is_within(&p, dir))
}

/// Deletes what's in a folder, the folder held open meanwhile so it can't be
/// swapped for a link half-way. A folder that can't be opened is counted,
/// unless it's the one to clean itself: the scan couldn't measure it either.
fn empty_dir(dir: &Path, rules: &Rules, report: &mut CleanReport, top: bool) {
    // Held until the folder is done.
    let _pin = match pin_dir(dir) {
        Ok(pin) => pin,
        Err(Blocked::Link) => {
            report.skipped_links += 1;
            return;
        }
        Err(Blocked::Gone) => return,
        Err(Blocked::Unreadable) => {
            report.unreadable_folders += u64::from(!top);
            return;
        }
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        report.unreadable_folders += u64::from(!top);
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if rules.skips(&path, &meta) {
            continue;
        }
        if meta.is_dir() {
            empty_dir(&path, rules, report, false);
            // Succeeds only once it's empty; one made today stays.
            if !rules.too_new(&meta) {
                let _ = std::fs::remove_dir(&path);
            }
        } else if !rules.too_new(&meta) {
            remove_file(&path, &meta, report);
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn recycle_bin() -> (u64, u64) {
        (0, 0)
    }

    pub fn empty_recycle_bin() -> bool {
        false
    }
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::UI::Shell::{
        SHEmptyRecycleBinW, SHQueryRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHQUERYRBINFO,
    };

    /// Size and item count of the Recycle Bin on all drives.
    pub fn recycle_bin() -> (u64, u64) {
        let mut info = SHQUERYRBINFO {
            cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
            ..Default::default()
        };
        if unsafe { SHQueryRecycleBinW(std::ptr::null(), &mut info) } != 0 {
            return (0, 0);
        }
        (info.i64Size.max(0) as u64, info.i64NumItems.max(0) as u64)
    }

    pub fn empty_recycle_bin() -> bool {
        let flags = SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND;
        unsafe { SHEmptyRecycleBinW(std::ptr::null_mut(), std::ptr::null(), flags) == 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_existing_folders_and_skips_missing_ones() {
        let dir = std::env::temp_dir().join(format!("uwiper-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("Default/Cache")).unwrap();
        std::fs::write(dir.join("Default/Cache/a.bin"), [0u8; 100]).unwrap();
        std::fs::write(dir.join("Default/Cache/b.bin"), [0u8; 50]).unwrap();
        std::env::set_var("UWIPER_CLEANUP_TEST", &dir);

        let def = Def {
            id: "t",
            group: "apps",
            name: "Test",
            description: "",
            default_on: true,
            specs: &[
                Spec::Chromium("%UWIPER_CLEANUP_TEST%"),
                Spec::Dir("%UWIPER_CLEANUP_TEST%/missing"),
            ],
        };
        let item = measure(&def).unwrap();
        assert_eq!(item.size_bytes, 150);
        assert_eq!(item.file_count, 2);
        assert_eq!(item.paths.len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn resolves_nested_folders_by_prefix() {
        let dir = std::env::temp_dir().join(format!("uwiper-tdata-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("user_data/cache")).unwrap();
        std::fs::create_dir_all(dir.join("user_data#2/media_cache")).unwrap();
        std::fs::create_dir_all(dir.join("D877F783D5D3EF8C/cache")).unwrap();
        std::env::set_var("UWIPER_TDATA_TEST", &dir);
        let mut found = resolve(&Spec::Children(
            "%UWIPER_TDATA_TEST%",
            "user_data",
            &["cache", "media_cache"],
        ));
        found.sort();
        let mut expected = vec![dir.join("user_data#2/media_cache"), dir.join("user_data/cache")];
        expected.sort();
        assert_eq!(found, expected);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn finds_app_caches_by_folder_name() {
        let root = std::env::temp_dir().join(format!("uwiper-detect-{}", std::process::id()));
        std::fs::create_dir_all(root.join("SomeApp/GPUCache")).unwrap();
        std::fs::create_dir_all(root.join("SomeApp/Partitions/main/Cache/Cache_Data")).unwrap();
        std::fs::create_dir_all(root.join("Known/Cache")).unwrap();
        std::fs::create_dir_all(root.join("NoCache/data")).unwrap();
        std::env::set_var("APPDATA", &root);
        std::env::set_var("LOCALAPPDATA", root.join("none"));
        std::env::set_var("USERPROFILE", root.join("none"));

        std::fs::create_dir_all(root.join("Thorium/User Data/Default/Cache")).unwrap();
        std::fs::write(root.join("Thorium/User Data/Local State"), b"{}").unwrap();
        std::fs::create_dir_all(root.join("Waterfox/Profiles/abc.default/cache2/entries")).unwrap();
        // Not a browser: "User Data" without Local State, and a cache holding envs.
        std::fs::create_dir_all(root.join("Tool/User Data/Default/Cache")).unwrap();
        std::fs::create_dir_all(root.join("Poetryish/Cache/virtualenvs/x")).unwrap();

        let known = vec![canonical(&root.join("Known/Cache").to_string_lossy())];
        let found = detect(&known, &[]);
        let names: Vec<&str> = found.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names.len(), 4, "{names:?}");
        assert!(names.contains(&"Tool – other caches"), "a plain Cache is only optional");
        assert!(!names.iter().any(|n| n.starts_with("Poetryish")));
        assert!(names.contains(&"Thorium – cache") && names.contains(&"Waterfox – cache"));
        let app = found.iter().find(|d| d.name == "SomeApp – cache").unwrap();
        assert!(app.default_on);
        assert_eq!(app.paths.len(), 2, "the nested Cache, not its Cache_Data");
        assert_eq!(
            package_name("5319275A.WhatsAppDesktop_cv1g1gvanyjgm"),
            "WhatsAppDesktop"
        );
        assert_eq!(
            package_name("PythonSoftwareFoundation.Python.3.12_qbz5n2kfra8p0"),
            "Python"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn empties_folders_but_keeps_them() {
        let dir = std::env::temp_dir().join(format!("uwiper-clean-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub/deeper")).unwrap();
        std::fs::write(dir.join("a.bin"), [0u8; 10]).unwrap();
        std::fs::write(dir.join("sub/deeper/b.bin"), [0u8; 5]).unwrap();
        let keep = dir.join("UWiper");
        std::fs::create_dir_all(&keep).unwrap();
        std::fs::write(keep.join("settings.toml"), b"x").unwrap();
        let own = vec![canonical(&keep.to_string_lossy())];
        // A browser profile caught in a cache folder keeps its sign-ins.
        std::fs::create_dir_all(dir.join("sub/Network")).unwrap();
        std::fs::write(dir.join("sub/Network/Cookies"), b"c").unwrap();
        std::fs::write(dir.join("sub/History"), b"h").unwrap();
        let mut report = CleanReport::default();
        empty_dir(&dir, &Rules::for_category("t", &own), &mut report, true);
        assert_eq!(report.freed_bytes, 15);
        assert_eq!(report.skipped_files, 0);
        assert!(dir.is_dir());
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            2,
            "UWiper's own folder and the folder with profile data are left"
        );
        assert!(keep.join("settings.toml").exists());
        assert!(dir.join("sub/Network/Cookies").exists() && dir.join("sub/History").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn embedded_browsers_give_only_their_caches() {
        let dir = std::env::temp_dir().join(format!("uwiper-cef-{}", std::process::id()));
        for sub in [
            "GPUCache",
            "Default/Cache/Cache_Data",
            "Default/Code Cache/js",
            "Default/Network",
            "Default/Local Storage",
        ] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
        }
        std::fs::write(dir.join("Default/Network/Cookies"), b"c").unwrap();
        std::fs::write(dir.join("Default/History"), b"h").unwrap();
        let mut found: Vec<String> = webview_caches(&dir)
            .iter()
            .map(|p| p.strip_prefix(&dir).unwrap().to_string_lossy().replace('\\', "/"))
            .collect();
        found.sort();
        assert_eq!(found, ["Default/Cache", "Default/Code Cache", "GPUCache"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn never_cleans_through_a_link() {
        let dir = std::env::temp_dir().join(format!("uwiper-link-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("valuable")).unwrap();
        std::fs::write(dir.join("valuable/keep.txt"), b"k").unwrap();
        std::os::unix::fs::symlink(dir.join("valuable"), dir.join("Cache")).unwrap();
        assert_eq!(reach(&dir.join("Cache")).err(), Some(Blocked::Link));
        assert_eq!(reach(&dir.join("Cache").join("inner")).err(), Some(Blocked::Link));
        assert!(reach(&dir.join("valuable")).is_ok());
        assert!(holds_user_files(Path::new("/")));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
