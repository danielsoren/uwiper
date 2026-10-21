//! Installed programs, read from the same Uninstall keys that Windows
//! Settings uses (machine-wide 64/32-bit and per-user).
#![cfg_attr(not(windows), allow(dead_code))]

use serde::Serialize;

use crate::error::{blocking, Result};
use crate::util::dir_size;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    id: String,
    name: String,
    publisher: Option<String>,
    version: Option<String>,
    install_location: Option<String>,
    /// ISO date, e.g. "2026-03-12"
    install_date: Option<String>,
    /// What the installer reported, not measured on disk.
    size_bytes: Option<u64>,
    /// "win32" or "msi"
    source: &'static str,
    /// "machine" or "user"
    scope: &'static str,
    arch: Option<&'static str>,
    uninstall_string: Option<String>,
    quiet_uninstall_string: Option<String>,
    registry_key: String,
}

#[tauri::command]
pub async fn list_apps() -> Result<Vec<InstalledApp>> {
    blocking(imp::list).await
}

/// Measures a folder on disk.
#[tauri::command]
pub async fn folder_size(path: String) -> Result<u64> {
    blocking(move || Ok(dir_size(std::path::Path::new(&path)).0)).await
}

/// `20260312` → `2026-03-12`
fn iso_date(raw: &str) -> Option<String> {
    let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
    if digits.len() != 8 {
        return None;
    }
    Some(format!("{}-{}-{}", &digits[..4], &digits[4..6], &digits[6..]))
}

fn is_update(release_type: &str) -> bool {
    matches!(release_type, "Update" | "Hotfix" | "Security Update" | "Service Pack")
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    use crate::error::Error;

    pub fn list() -> Result<Vec<InstalledApp>> {
        Err(Error::Unsupported)
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::collections::HashSet;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};
    use winreg::RegKey;

    const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";

    pub fn list() -> Result<Vec<InstalledApp>> {
        let sources = [
            (HKEY_LOCAL_MACHINE, "HKLM", KEY_WOW64_64KEY, "machine", Some("x64")),
            (HKEY_LOCAL_MACHINE, "HKLM", KEY_WOW64_32KEY, "machine", Some("x86")),
            (HKEY_CURRENT_USER, "HKCU", 0, "user", None),
        ];
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for (hive, hive_name, view, scope, arch) in sources {
            let Ok(root) = RegKey::predef(hive).open_subkey_with_flags(UNINSTALL, KEY_READ | view) else {
                continue;
            };
            for key_name in root.enum_keys().flatten() {
                let Ok(key) = root.open_subkey_with_flags(&key_name, KEY_READ | view) else {
                    continue;
                };
                let Some(app) = read(&key, &key_name, hive_name, view, scope, arch) else {
                    continue;
                };
                let dedupe = (app.name.to_lowercase(), app.version.clone(), app.publisher.clone());
                if seen.insert(dedupe) {
                    out.push(app);
                }
            }
        }
        out.sort_by_key(|a| a.name.to_lowercase());
        Ok(out)
    }

    fn read(
        key: &RegKey,
        key_name: &str,
        hive: &str,
        view: u32,
        scope: &'static str,
        arch: Option<&'static str>,
    ) -> Option<InstalledApp> {
        let text = |name: &str| {
            key.get_value::<String, _>(name)
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        };
        let dword = |name: &str| key.get_value::<u32, _>(name).ok();

        let name = text("DisplayName")?;
        // Hidden components, updates and patches are not separate programs.
        if dword("SystemComponent") == Some(1)
            || text("ParentKeyName").is_some()
            || text("ReleaseType").is_some_and(|t| is_update(&t))
        {
            return None;
        }
        let uninstall_string = text("UninstallString");
        let quiet_uninstall_string = text("QuietUninstallString");
        if uninstall_string.is_none() && quiet_uninstall_string.is_none() {
            return None;
        }
        let view_suffix = if view == KEY_WOW64_32KEY { " (32-bit view)" } else { "" };
        Some(InstalledApp {
            id: format!("{hive}|{view}|{key_name}"),
            publisher: text("Publisher"),
            version: text("DisplayVersion"),
            install_location: text("InstallLocation").map(|p| p.trim_matches('"').trim_end_matches('\\').to_string()),
            install_date: text("InstallDate").as_deref().and_then(iso_date),
            size_bytes: dword("EstimatedSize").filter(|&kb| kb > 0).map(|kb| u64::from(kb) * 1024),
            source: if dword("WindowsInstaller") == Some(1) { "msi" } else { "win32" },
            scope,
            arch,
            uninstall_string,
            quiet_uninstall_string,
            registry_key: format!(r"{hive}\{UNINSTALL}\{key_name}{view_suffix}"),
            name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_install_dates() {
        assert_eq!(iso_date("20260312").as_deref(), Some("2026-03-12"));
        assert_eq!(iso_date("2026-03-12").as_deref(), Some("2026-03-12"));
        assert_eq!(iso_date("3/12/2026"), None);
    }

    #[test]
    fn skips_updates() {
        assert!(is_update("Security Update"));
        assert!(!is_update("Application"));
    }
}
