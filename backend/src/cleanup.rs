//! Caches and temporary files that Windows and apps rebuild on their own.
//! This version only measures them; nothing is deleted.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{blocking, Result};
use crate::util::{dir_size, expand_env};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CleanupItem {
    id: &'static str,
    group: &'static str,
    name: &'static str,
    description: &'static str,
    default_on: bool,
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
    /// Cache folders of every profile in a Chromium "User Data" folder.
    Chromium(&'static str),
    /// `cache2` of every Firefox profile.
    Firefox(&'static str),
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

#[rustfmt::skip]
const DEFS: &[Def] = &[
    Def { id: "temp", group: "windows", name: "Temporary files", description: "%TEMP% and Windows\\Temp", default_on: true,
        specs: &[Spec::Dir("%TEMP%"), Spec::Dir(r"%SystemRoot%\Temp")] },
    Def { id: "windows-update", group: "windows", name: "Windows Update cache", description: "Downloaded packages for updates that are already installed", default_on: true,
        specs: &[Spec::Dir(r"%SystemRoot%\SoftwareDistribution\Download")] },
    Def { id: "delivery-optimization", group: "windows", name: "Delivery Optimization", description: "Update files shared with other PCs", default_on: true,
        specs: &[Spec::Dir(r"%SystemRoot%\ServiceProfiles\NetworkService\AppData\Local\Microsoft\Windows\DeliveryOptimization\Cache")] },
    Def { id: "crash-dumps", group: "windows", name: "Crash reports & memory dumps", description: "Windows Error Reporting, minidumps", default_on: true,
        specs: &[
            Spec::Dir(r"%ProgramData%\Microsoft\Windows\WER\ReportArchive"),
            Spec::Dir(r"%ProgramData%\Microsoft\Windows\WER\ReportQueue"),
            Spec::Dir(r"%LOCALAPPDATA%\CrashDumps"),
            Spec::Dir(r"%SystemRoot%\Minidump"),
            Spec::File(r"%SystemRoot%\MEMORY.DMP"),
        ] },
    Def { id: "shader-cache", group: "windows", name: "Shader caches", description: "DirectX and GPU driver caches, rebuilt by games", default_on: true,
        specs: &[
            Spec::Dir(r"%LOCALAPPDATA%\D3DSCache"),
            Spec::Dir(r"%LOCALAPPDATA%\NVIDIA\DXCache"),
            Spec::Dir(r"%LOCALAPPDATA%\NVIDIA\GLCache"),
            Spec::Dir(r"%LOCALAPPDATA%\AMD\DxCache"),
        ] },
    Def { id: "thumbnails", group: "windows", name: "Thumbnail cache", description: "Explorer previews, rebuilt on demand", default_on: true,
        specs: &[Spec::Prefixed(r"%LOCALAPPDATA%\Microsoft\Windows\Explorer", "thumbcache_")] },

    Def { id: "chrome", group: "browsers", name: "Google Chrome – cache", description: BROWSER_NOTE, default_on: true,
        specs: &[Spec::Chromium(r"%LOCALAPPDATA%\Google\Chrome\User Data")] },
    Def { id: "edge", group: "browsers", name: "Microsoft Edge – cache", description: BROWSER_NOTE, default_on: true,
        specs: &[Spec::Chromium(r"%LOCALAPPDATA%\Microsoft\Edge\User Data")] },
    Def { id: "brave", group: "browsers", name: "Brave – cache", description: BROWSER_NOTE, default_on: true,
        specs: &[Spec::Chromium(r"%LOCALAPPDATA%\BraveSoftware\Brave-Browser\User Data")] },
    Def { id: "firefox", group: "browsers", name: "Firefox – cache", description: BROWSER_NOTE, default_on: true,
        specs: &[Spec::Firefox(r"%LOCALAPPDATA%\Mozilla\Firefox\Profiles")] },

    Def { id: "npm", group: "developer", name: "npm cache", description: REDOWNLOAD, default_on: false,
        specs: &[Spec::Dir(r"%LOCALAPPDATA%\npm-cache")] },
    Def { id: "pnpm", group: "developer", name: "pnpm cache", description: "Metadata cache; the package store is left alone", default_on: false,
        specs: &[Spec::Dir(r"%LOCALAPPDATA%\pnpm-cache")] },
    Def { id: "yarn", group: "developer", name: "Yarn cache", description: REDOWNLOAD, default_on: false,
        specs: &[Spec::Dir(r"%LOCALAPPDATA%\Yarn\Cache")] },
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
    Def { id: "gradle", group: "developer", name: "Gradle caches", description: REDOWNLOAD, default_on: false,
        specs: &[Spec::Dir(r"%USERPROFILE%\.gradle\caches")] },

    Def { id: "discord", group: "apps", name: "Discord – cache", description: "Images and media from chats", default_on: true,
        specs: &[Spec::Dir(r"%APPDATA%\discord\Cache"), Spec::Dir(r"%APPDATA%\discord\Code Cache"), Spec::Dir(r"%APPDATA%\discord\GPUCache")] },
    Def { id: "slack", group: "apps", name: "Slack – cache", description: "Images and media from chats", default_on: true,
        specs: &[Spec::Dir(r"%APPDATA%\Slack\Cache"), Spec::Dir(r"%APPDATA%\Slack\Code Cache"), Spec::Dir(r"%APPDATA%\Slack\GPUCache")] },
    Def { id: "vscode", group: "apps", name: "VS Code – cache", description: "Rebuilt when VS Code starts", default_on: true,
        specs: &[
            Spec::Dir(r"%APPDATA%\Code\Cache"),
            Spec::Dir(r"%APPDATA%\Code\CachedData"),
            Spec::Dir(r"%APPDATA%\Code\Code Cache"),
            Spec::Dir(r"%APPDATA%\Code\GPUCache"),
        ] },
    Def { id: "spotify", group: "apps", name: "Spotify – cache", description: "Streaming cache; offline downloads are kept", default_on: false,
        specs: &[Spec::Dir(r"%LOCALAPPDATA%\Spotify\Data")] },
];

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|it| it.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default()
}

fn resolve(spec: &Spec) -> Vec<PathBuf> {
    let existing = |p: PathBuf| p.exists().then_some(p);
    match spec {
        Spec::Dir(p) | Spec::File(p) => existing(PathBuf::from(expand_env(p))).into_iter().collect(),
        Spec::Prefixed(dir, prefix) => std::fs::read_dir(expand_env(dir))
            .map(|it| {
                it.flatten()
                    .filter(|e| e.file_name().to_string_lossy().starts_with(prefix))
                    .map(|e| e.path())
                    .collect()
            })
            .unwrap_or_default(),
        Spec::Chromium(user_data) => {
            let root = PathBuf::from(expand_env(user_data));
            let mut out: Vec<PathBuf> = ["GrShaderCache", "ShaderCache"]
                .iter()
                .filter_map(|d| existing(root.join(d)))
                .collect();
            for profile in subdirs(&root) {
                for d in ["Cache", "Code Cache", "GPUCache"] {
                    out.extend(existing(profile.join(d)));
                }
            }
            out
        }
        Spec::Firefox(profiles) => subdirs(Path::new(&expand_env(profiles)))
            .into_iter()
            .filter_map(|p| existing(p.join("cache2")))
            .collect(),
    }
}

fn measure(def: &Def) -> Option<CleanupItem> {
    let paths: Vec<PathBuf> = def.specs.iter().flat_map(resolve).collect();
    if paths.is_empty() {
        return None;
    }
    let (mut size_bytes, mut file_count) = (0, 0);
    for p in &paths {
        if p.is_file() {
            size_bytes += p.metadata().map(|m| m.len()).unwrap_or(0);
            file_count += 1;
        } else {
            let (bytes, files) = dir_size(p);
            size_bytes += bytes;
            file_count += files;
        }
    }
    Some(CleanupItem {
        id: def.id,
        group: def.group,
        name: def.name,
        description: def.description,
        default_on: def.default_on,
        size_bytes,
        file_count,
        paths: paths.iter().map(|p| p.to_string_lossy().into_owned()).collect(),
    })
}

#[tauri::command]
pub async fn scan_cleanup() -> Result<Vec<CleanupItem>> {
    blocking(|| {
        // Each category walks its own folders, so measure them in parallel.
        let items = std::thread::scope(|scope| {
            let handles: Vec<_> = DEFS.iter().map(|def| scope.spawn(move || measure(def))).collect();
            handles.into_iter().filter_map(|h| h.join().ok().flatten()).collect()
        });
        Ok(items)
    })
    .await
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
}
