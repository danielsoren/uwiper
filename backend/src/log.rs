//! A plain-text record of what UWiper changed, one file per day in the logs
//! folder. Old files are removed after the number of days set in Settings.

use std::io::Write;

use crate::paths::data_dir;
use crate::settings::Settings;
use crate::util::{now_secs, utc_date};

pub fn write(settings: &Settings, lines: &[String]) {
    let dir = data_dir(settings, "logs");
    if lines.is_empty() || std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let now = now_secs();
    let time = format!("{:02}:{:02}:{:02}", now / 3600 % 24, now / 60 % 60, now % 60);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(format!("{}.log", utc_date(now))));
    if let Ok(mut file) = file {
        for line in lines {
            let _ = writeln!(file, "{time} UTC  {line}");
        }
    }
}

pub fn purge(settings: &Settings) {
    let max_age = std::time::Duration::from_secs(u64::from(settings.storage.log_days) * 86_400);
    let Ok(entries) = std::fs::read_dir(data_dir(settings, "logs")) else {
        return;
    };
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .is_ok_and(|t| t.elapsed().is_ok_and(|age| age > max_age));
        if old && entry.path().extension().is_some_and(|e| e == "log") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}
