//! Third-party driver packages in the Driver Store. Windows keeps every
//! version a driver update brings; the older ones no device uses can go.
#![cfg_attr(not(windows), allow(dead_code))]

use std::collections::HashMap;

use serde::Serialize;

use crate::error::{blocking, Result};
use crate::targets::Target;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DriverItem {
    /// The published name, such as `oem12.inf`.
    id: String,
    /// The package's own INF name, such as `nv_dispi.inf`.
    inf: Option<String>,
    provider: String,
    class: String,
    version: Option<String>,
    /// `yyyy-mm-dd`
    date: Option<String>,
    size_bytes: Option<u64>,
    /// A device it drives – or, for an older version, one its newest version drives.
    device: Option<String>,
    /// "used": a device (connected or not) is set up with it, or a service or
    /// printer driver runs from it. "unused": none of that. "unknown": UWiper
    /// can't tell (extension packages, a package it can't locate) – never removed.
    usage: &'static str,
    /// A newer version of the same package is in the Driver Store.
    old: bool,
    /// Safe to pick for removal: an older version nothing uses, while a device
    /// uses the newest one – proof the device moved on.
    suggested: bool,
    removal: Target,
    /// Its Driver Store folder and use when listed, checked again before removing.
    check: Option<String>,
}

#[tauri::command]
pub async fn list_drivers() -> Result<Vec<DriverItem>> {
    blocking(list).await
}

pub fn list() -> Result<Vec<DriverItem>> {
    imp::list()
}

/// Where a published package lives: its Driver Store folder and its own INF name.
pub(crate) fn store_location(inf: &str) -> Option<(String, String)> {
    imp::store_location(inf)
}

/// A package's Driver Store folder and use, to tell before removing it that
/// it's still the same package and still unused.
pub(crate) fn fingerprint(inf: &str) -> Option<String> {
    imp::fingerprint(inf)
}

/// Extension packages add to a device's driver without being its driver, so
/// device keys don't show their use.
const USE_UNKNOWABLE: [&str; 1] = ["extension"];

fn usage(devices: bool, used: bool, located: bool, class: &str) -> &'static str {
    if devices || used {
        "used"
    } else if !located || USE_UNKNOWABLE.contains(&class.to_lowercase().as_str()) {
        "unknown"
    } else {
        "unused"
    }
}

fn check_of(folder: Option<&str>, usage: &str) -> Option<String> {
    folder.map(|f| format!("d:{}:{usage}", f.to_lowercase()))
}

/// What the [Version] section of an INF says.
#[derive(Debug, Default, PartialEq)]
struct InfInfo {
    class: Option<String>,
    provider: Option<String>,
    version: Option<String>,
    date: Option<String>,
}

/// Drops a `;` comment, unless the `;` is inside quotes.
fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    for (i, ch) in line.char_indices() {
        match ch {
            '"' => quoted = !quoted,
            ';' if !quoted => return &line[..i],
            _ => {}
        }
    }
    line
}

fn parse_inf(text: &str) -> InfInfo {
    let mut section = String::new();
    let mut version: HashMap<String, String> = HashMap::new();
    // [Strings] first, then US English, then any other language.
    let mut strings: [HashMap<String, String>; 3] = Default::default();
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.trim().to_ascii_lowercase();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim().to_ascii_lowercase(), value.trim().to_string());
        let table = match section.as_str() {
            "version" => &mut version,
            "strings" => &mut strings[0],
            "strings.0409" => &mut strings[1],
            s if s.starts_with("strings.") => &mut strings[2],
            _ => continue,
        };
        table.entry(key).or_insert(value);
    }

    let unquote = |v: &str| {
        v.strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(v)
            .to_string()
    };
    // `%Name%` stands for a [Strings] entry; `%%` is a percent sign.
    let resolve = |value: &str| -> String {
        let mut out = String::new();
        let mut parts = value.split('%');
        out.push_str(&unquote(parts.next().unwrap_or_default()));
        let mut rest: Vec<&str> = parts.collect();
        while rest.len() >= 2 {
            let (token, after) = (rest[0], rest[1]);
            if token.is_empty() {
                out.push('%');
            } else {
                let key = token.to_ascii_lowercase();
                match strings.iter().find_map(|t| t.get(&key)) {
                    Some(s) => out.push_str(&unquote(s)),
                    None => out.push_str(token),
                }
            }
            out.push_str(&unquote(after));
            rest.drain(..2);
        }
        out.trim().to_string()
    };
    let field = |key: &str| version.get(key).map(|v| resolve(v)).filter(|v| !v.is_empty());

    let (date, ver) = match field("driverver") {
        Some(dv) => {
            let (date, ver) = dv.split_once(',').unwrap_or((dv.as_str(), ""));
            let ver = ver.trim();
            (us_date(date.trim()), (!ver.is_empty()).then(|| ver.to_string()))
        }
        None => (None, None),
    };
    InfInfo {
        class: field("class"),
        provider: field("provider"),
        version: ver,
        date,
    }
}

/// `mm/dd/yyyy` to `yyyy-mm-dd`.
fn us_date(text: &str) -> Option<String> {
    let mut parts = text.split(['/', '-']).map(|p| p.trim().parse::<u32>().ok());
    let (m, d, y) = (parts.next()??, parts.next()??, parts.next()??);
    ((1..=12).contains(&m) && (1..=31).contains(&d) && (1980..=2200).contains(&y))
        .then(|| format!("{y:04}-{m:02}-{d:02}"))
}

fn version_key(v: Option<&str>) -> Vec<u32> {
    v.unwrap_or_default()
        .split('.')
        .map(|p| p.trim().parse().unwrap_or(0))
        .collect()
}

/// Device Manager's names for the setup classes drivers use most.
fn class_label(class: &str) -> String {
    let label = match class.to_ascii_lowercase().as_str() {
        "display" => "Display adapters",
        "net" => "Network adapters",
        "media" => "Sound, video and game controllers",
        "audioendpoint" => "Audio inputs and outputs",
        "audioprocessingobject" => "Audio processing objects",
        "system" => "System devices",
        "usb" => "USB controllers",
        "usbdevice" => "USB devices",
        "hidclass" => "Human interface devices",
        "bluetooth" => "Bluetooth",
        "softwarecomponent" => "Software components",
        "softwaredevice" => "Software devices",
        "extension" => "Extensions",
        "printer" => "Printers",
        "printqueue" => "Print queues",
        "monitor" => "Monitors",
        "keyboard" => "Keyboards",
        "mouse" => "Mice and other pointing devices",
        "image" => "Imaging devices",
        "camera" => "Cameras",
        "diskdrive" => "Disk drives",
        "scsiadapter" => "Storage controllers",
        "hdc" => "IDE ATA/ATAPI controllers",
        "ports" => "Ports (COM & LPT)",
        "biometric" => "Biometric devices",
        "securitydevices" => "Security devices",
        "firmware" => "Firmware",
        "sensor" => "Sensors",
        "wpd" => "Portable devices",
        "battery" => "Batteries",
        "netservice" => "Network services",
        "nettrans" => "Network protocols",
        "netclient" => "Network clients",
        "smartcardreader" => "Smart card readers",
        "processor" => "Processors",
        "mtd" => "Memory technology devices",
        "legacydriver" => "Non-Plug and Play drivers",
        "volume" => "Storage volumes",
        "xboxcomposite" => "Xbox peripherals",
        "" => "Other devices",
        _ => return class.to_string(),
    };
    label.to_string()
}

/// One package as read from disk, before old versions are worked out.
struct Package {
    id: String,
    inf: Option<String>,
    info: InfInfo,
    size_bytes: Option<u64>,
    devices: Vec<String>,
    /// A service or a printer driver runs from its Driver Store folder.
    used: bool,
    folder: Option<String>,
}

/// Printers, extension INFs and non-Plug and Play drivers can be in use in
/// ways no device key shows, so they're never picked for removal.
const NEVER_SUGGESTED: [&str; 4] = ["printer", "printqueue", "extension", "legacydriver"];

/// The Driver Store folder a path points into, lowercase.
fn store_folder_in(path: &str) -> Option<String> {
    let lower = path.to_lowercase().replace('/', "\\");
    let marker = r"\driverstore\filerepository\";
    let start = lower.find(marker)? + marker.len();
    let folder = lower[start..].split('\\').next()?;
    (!folder.is_empty()).then(|| folder.to_string())
}

/// Marks every package that has a newer version of itself, and lends the
/// newest one's device name to the older ones.
fn finish(packages: Vec<Package>) -> Vec<DriverItem> {
    let group_of = |p: &Package| {
        p.inf.as_ref().map(|inf| {
            (
                inf.to_lowercase(),
                p.info.provider.clone().unwrap_or_default().to_lowercase(),
                p.info.class.clone().unwrap_or_default().to_lowercase(),
            )
        })
    };
    let rank = |p: &Package| {
        (
            p.info.date.clone().unwrap_or_default(),
            version_key(p.info.version.as_deref()),
        )
    };
    let mut newest: HashMap<(String, String, String), usize> = HashMap::new();
    for (i, p) in packages.iter().enumerate() {
        if let Some(group) = group_of(p) {
            let entry = newest.entry(group).or_insert(i);
            if rank(p) > rank(&packages[*entry]) {
                *entry = i;
            }
        }
    }

    let mut out: Vec<DriverItem> = packages
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let top = group_of(p).and_then(|g| newest.get(&g).copied());
            let old = top.is_some_and(|t| t != i && rank(&packages[t]) > rank(p));
            let named = |p: &Package| p.devices.iter().find(|n| !n.is_empty()).cloned();
            let device = named(p).or_else(|| top.and_then(|t| named(&packages[t])));
            let class = p.info.class.as_deref().unwrap_or_default().to_lowercase();
            let usage = usage(!p.devices.is_empty(), p.used, p.folder.is_some(), &class);
            let suggested = old
                && usage == "unused"
                && top.is_some_and(|t| !packages[t].devices.is_empty())
                && !NEVER_SUGGESTED.contains(&class.as_str());
            DriverItem {
                id: p.id.clone(),
                inf: p.inf.clone(),
                provider: p.info.provider.clone().unwrap_or_else(|| "Unknown provider".into()),
                class: class_label(p.info.class.as_deref().unwrap_or_default()),
                version: p.info.version.clone(),
                date: p.info.date.clone(),
                size_bytes: p.size_bytes,
                device,
                usage,
                old,
                suggested,
                removal: Target::Driver { inf: p.id.clone() },
                check: check_of(p.folder.as_deref(), usage),
            }
        })
        .collect();
    out.sort_by(|a, b| {
        (
            a.provider.to_lowercase(),
            &a.class,
            a.inf.as_deref().unwrap_or(&a.id).to_lowercase(),
        )
            .cmp(&(
                b.provider.to_lowercase(),
                &b.class,
                b.inf.as_deref().unwrap_or(&b.id).to_lowercase(),
            ))
            .then_with(|| b.date.cmp(&a.date))
    });
    out
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    use crate::error::Error;

    pub fn list() -> Result<Vec<DriverItem>> {
        Err(Error::Unsupported)
    }

    pub fn store_location(_inf: &str) -> Option<(String, String)> {
        None
    }

    pub fn fingerprint(_inf: &str) -> Option<String> {
        None
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::targets::is_published_inf;
    use crate::util::{decode_text, dir_size, parallel_map, windows_dir};
    use std::collections::HashSet;
    use std::path::PathBuf;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    pub fn list() -> Result<Vec<DriverItem>> {
        let windows = windows_dir();
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        // Each published INF with its Driver Store folder, such as `nv_dispi.inf_amd64_1a2b3c4d`.
        let mut published: Vec<(PathBuf, String, Option<String>)> =
            std::fs::read_dir(PathBuf::from(&windows).join("INF"))?
                .flatten()
                .filter_map(|e| {
                    let id = e.file_name().to_string_lossy().into_owned();
                    is_published_inf(&id).then(|| {
                        let folder = store_location(&id).map(|(folder, _)| folder);
                        (e.path(), id, folder)
                    })
                })
                .collect();
        published.sort();
        let store = PathBuf::from(&windows).join(r"System32\DriverStore\FileRepository");
        let devices = devices(&hklm);
        let users = store_users(&hklm);

        let packages = parallel_map(&published, |(path, id, folder)| {
            let info = parse_inf(&decode_text(&std::fs::read(path).ok()?));
            let inf = folder.as_deref().and_then(|f| {
                f.to_lowercase()
                    .find(".inf_")
                    .and_then(|end| f.get(..end + 4))
                    .map(str::to_string)
            });
            let size_bytes = folder
                .as_ref()
                .map(|f| store.join(f))
                .filter(|p| p.is_dir())
                .map(|p| dir_size(&p).0);
            Some(Package {
                devices: devices.get(&id.to_lowercase()).cloned().unwrap_or_default(),
                used: folder.as_ref().is_some_and(|f| users.contains(&f.to_lowercase())),
                folder: folder.clone(),
                id: id.clone(),
                inf,
                info,
                size_bytes,
            })
        });
        Ok(finish(packages.into_iter().flatten().collect()))
    }

    /// Through SetupAPI, which knows every package (the registry's driver
    /// database doesn't list them all); the registry when that fails.
    pub fn store_location(inf: &str) -> Option<(String, String)> {
        use windows_sys::Win32::Devices::DeviceAndDriverInstallation::SetupGetInfDriverStoreLocationW;
        let name: Vec<u16> = inf.encode_utf16().chain(Some(0)).collect();
        let mut buf = vec![0u16; 1024];
        let mut needed = 0u32;
        let ok = unsafe {
            SetupGetInfDriverStoreLocationW(
                name.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                buf.as_mut_ptr(),
                buf.len() as u32,
                &mut needed,
            )
        };
        let from_api = (ok != 0).then(|| {
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            let path = PathBuf::from(String::from_utf16_lossy(&buf[..len]));
            let file = path.file_name()?.to_string_lossy().into_owned();
            let folder = path.parent()?.file_name()?.to_string_lossy().into_owned();
            Some((folder, file))
        });
        from_api.flatten().or_else(|| {
            let key = RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey_with_flags(format!(r"SYSTEM\DriverDatabase\DriverInfFiles\{inf}"), KEY_READ)
                .ok()?;
            let folder = key
                .get_value::<String, _>("Active")
                .ok()
                .or_else(|| key.get_value::<Vec<String>, _>("").ok()?.into_iter().next())
                .filter(|f| !f.is_empty() && !f.contains(['\\', '/']))?;
            let end = folder.to_lowercase().find(".inf_")?;
            let file = folder.get(..end + 4)?.to_string();
            Some((folder, file))
        })
    }

    pub fn fingerprint(inf: &str) -> Option<String> {
        let folder = store_location(inf).map(|(folder, _)| folder);
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let devices = devices(&hklm).get(&inf.to_lowercase()).is_some_and(|d| !d.is_empty());
        let used = folder
            .as_ref()
            .is_some_and(|f| store_users(&hklm).contains(&f.to_lowercase()));
        let text = std::fs::read(PathBuf::from(windows_dir()).join("INF").join(inf)).ok()?;
        let class = parse_inf(&decode_text(&text)).class.unwrap_or_default();
        check_of(folder.as_deref(), usage(devices, used, folder.is_some(), &class))
    }

    /// Driver Store folders that services and printer drivers run from.
    fn store_users(hklm: &RegKey) -> HashSet<String> {
        let mut out = HashSet::new();
        let open = |key: &RegKey, name: &str| key.open_subkey_with_flags(name, KEY_READ);
        if let Ok(services) = open(hklm, r"SYSTEM\CurrentControlSet\Services") {
            for name in services.enum_keys().flatten() {
                let image = open(&services, &name).and_then(|k| k.get_value::<String, _>("ImagePath"));
                out.extend(image.ok().as_deref().and_then(store_folder_in));
            }
        }
        if let Ok(environments) = open(hklm, r"SYSTEM\CurrentControlSet\Control\Print\Environments") {
            for environment in environments.enum_keys().flatten() {
                let Ok(drivers) = open(&environments, &format!(r"{environment}\Drivers")) else {
                    continue;
                };
                for version in drivers.enum_keys().flatten() {
                    let Ok(version_key) = open(&drivers, &version) else {
                        continue;
                    };
                    for driver in version_key.enum_keys().flatten() {
                        let inf = open(&version_key, &driver).and_then(|k| k.get_value::<String, _>("InfPath"));
                        out.extend(inf.ok().as_deref().and_then(store_folder_in));
                    }
                }
            }
        }
        out
    }

    /// Published INF name (lowercase) to the names of the devices set up with it,
    /// connected or not, from the device class keys.
    fn devices(hklm: &RegKey) -> HashMap<String, Vec<String>> {
        let mut out: HashMap<String, Vec<String>> = HashMap::new();
        let Ok(classes) = hklm.open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Control\Class", KEY_READ) else {
            return out;
        };
        for class in classes.enum_keys().flatten() {
            let Ok(class_key) = classes.open_subkey_with_flags(&class, KEY_READ) else {
                continue;
            };
            for device in class_key.enum_keys().flatten() {
                let Ok(key) = class_key.open_subkey_with_flags(&device, KEY_READ) else {
                    continue;
                };
                let Ok(inf) = key.get_value::<String, _>("InfPath") else {
                    continue;
                };
                if !is_published_inf(&inf) {
                    continue;
                }
                // A nameless entry still marks the package as in use.
                let name = key
                    .get_value::<String, _>("DriverDesc")
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let names = out.entry(inf.to_lowercase()).or_default();
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INF: &str = r#"
; Copyright
[Version]
Signature   = "$WINDOWS NT$"
Class       = Display ; adapters
ClassGUID   = {4d36e968-e325-11ce-bfc1-08002be10318}
Provider    = %NVIDIA%
DriverVer   = 3/7/2024, 31.0.15.5222
CatalogFile = NV_DISP.CAT

[Strings.0407]
NVIDIA = "NVIDIA (de)"

[Strings]
NVIDIA = "NVIDIA; Corp"
"#;

    #[test]
    fn reads_the_version_section() {
        let info = parse_inf(INF);
        assert_eq!(info.class.as_deref(), Some("Display"));
        assert_eq!(info.provider.as_deref(), Some("NVIDIA; Corp"));
        assert_eq!(info.version.as_deref(), Some("31.0.15.5222"));
        assert_eq!(info.date.as_deref(), Some("2024-03-07"));
        assert_eq!(
            parse_inf("[Version]\nProvider=100%% Tools\n").provider.as_deref(),
            Some("100% Tools")
        );
        assert_eq!(
            parse_inf("[version]\nprovider=%Missing%\n").provider.as_deref(),
            Some("Missing")
        );
    }

    fn package(id: &str, inf: &str, date: &str, version: &str, devices: &[&str]) -> Package {
        Package {
            id: id.into(),
            inf: Some(inf.into()),
            info: InfInfo {
                class: Some("Display".into()),
                provider: Some("NVIDIA".into()),
                version: Some(version.into()),
                date: Some(date.into()),
            },
            size_bytes: Some(1),
            devices: devices.iter().map(|d| d.to_string()).collect(),
            used: false,
            folder: Some(format!("{inf}_amd64_{id}")),
        }
    }

    #[test]
    fn older_versions_are_marked_and_named_after_the_newest() {
        let items = finish(vec![
            package("oem3.inf", "nv_dispi.inf", "2023-01-10", "31.0.15.3000", &[]),
            package(
                "oem9.inf",
                "nv_dispi.inf",
                "2024-03-07",
                "31.0.15.5222",
                &["NVIDIA GeForce RTX 3080"],
            ),
            package("oem5.inf", "nv_dispi.inf", "2023-01-10", "31.0.15.2999", &[]),
            package(
                "oem7.inf",
                "nvhda.inf",
                "2022-05-01",
                "1.3.39.14",
                &["NVIDIA High Definition Audio"],
            ),
        ]);
        let get = |id: &str| items.iter().find(|i| i.id == id).unwrap();
        assert!(!get("oem9.inf").old && get("oem9.inf").usage == "used");
        assert!(get("oem3.inf").old && get("oem3.inf").usage == "unused" && get("oem3.inf").suggested);
        assert!(get("oem5.inf").old);
        assert!(!get("oem7.inf").old, "the only version of its package");
        assert_eq!(get("oem3.inf").device.as_deref(), Some("NVIDIA GeForce RTX 3080"));
        let nameless = finish(vec![package("oem1.inf", "x.inf", "2024-01-01", "1.0", &[""])]);
        assert!(nameless[0].usage == "used" && nameless[0].device.is_none());
        assert_eq!(get("oem7.inf").class, "Display adapters");
    }

    #[test]
    fn picks_only_what_nothing_uses() {
        let mut printer = package("oem2.inf", "hpprint.inf", "2021-01-01", "1.0", &[]);
        printer.info.class = Some("Printer".into());
        let mut printer_new = package("oem4.inf", "hpprint.inf", "2022-01-01", "2.0", &["HP LaserJet"]);
        printer_new.info.class = Some("Printer".into());
        let mut serviced = package("oem6.inf", "vpn.inf", "2021-01-01", "1.0", &[]);
        serviced.used = true;
        let items = finish(vec![
            printer,
            printer_new,
            serviced,
            package("oem8.inf", "vpn.inf", "2022-01-01", "2.0", &["VPN adapter"]),
            // The newest version isn't on any device either: nothing proves the old one is spare.
            package("oem10.inf", "cam.inf", "2021-01-01", "1.0", &[]),
            package("oem11.inf", "cam.inf", "2022-01-01", "2.0", &[]),
        ]);
        let get = |id: &str| items.iter().find(|i| i.id == id).unwrap();
        assert!(
            get("oem2.inf").old && !get("oem2.inf").suggested,
            "printers are never picked"
        );
        assert!(
            get("oem6.inf").usage == "used" && !get("oem6.inf").suggested,
            "a service runs from it"
        );
        assert!(get("oem10.inf").old && !get("oem10.inf").suggested);
    }

    #[test]
    fn extension_packages_and_unlocated_ones_are_unknown() {
        let mut ext = package("oem3.inf", "ext.inf", "2021-01-01", "1.0", &[]);
        ext.info.class = Some("Extension".into());
        let mut lost = package("oem4.inf", "lost.inf", "2021-01-01", "1.0", &[]);
        lost.folder = None;
        let items = finish(vec![ext, lost]);
        assert!(items.iter().all(|i| i.usage == "unknown" && !i.suggested));
        assert!(items.iter().find(|i| i.id == "oem4.inf").unwrap().check.is_none());
    }

    #[test]
    fn finds_driver_store_folders_in_paths() {
        assert_eq!(
            store_folder_in(r"\SystemRoot\System32\DriverStore\FileRepository\vpn.inf_amd64_ab12\vpn.sys").as_deref(),
            Some("vpn.inf_amd64_ab12")
        );
        assert_eq!(
            store_folder_in(r"C:\Windows\System32\DriverStore\FileRepository\hp.inf_amd64_9f\hp.inf").as_deref(),
            Some("hp.inf_amd64_9f")
        );
        assert_eq!(store_folder_in(r"C:\Windows\System32\drivers\x.sys"), None);
    }

    #[test]
    fn dates_and_versions() {
        assert_eq!(us_date("12/31/2021").as_deref(), Some("2021-12-31"));
        assert_eq!(us_date("31/12/2021"), None);
        assert!(version_key(Some("10.0.2.10")) > version_key(Some("10.0.2.9")));
    }
}
