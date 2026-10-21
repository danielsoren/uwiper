//! Small helpers shared by the scanners: environment expansion, command-line
//! parsing and file checks.

use std::path::Path;

/// Expands `%VAR%` references. Unknown variables are left as they are.
pub fn expand_env(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('%') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let name = &after[..end];
        match std::env::var(name) {
            Ok(value) if !name.is_empty() => out.push_str(&value),
            _ => {
                out.push('%');
                out.push_str(name);
                out.push('%');
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Pulls the executable out of a command line such as
/// `"C:\Program Files\App\app.exe" --minimized` or `C:\Tools\app.exe -x`.
pub fn command_executable(command: &str) -> Option<String> {
    let s = command.trim();
    if s.is_empty() {
        return None;
    }
    let raw = if let Some(rest) = s.strip_prefix('"') {
        rest.split('"').next()?.to_string()
    } else {
        // Unquoted paths may contain spaces: take the shortest run of words
        // that ends in an executable extension, else just the first word.
        const EXTS: [&str; 6] = [".exe", ".com", ".bat", ".cmd", ".dll", ".sys"];
        let words: Vec<&str> = s.split(' ').collect();
        (1..=words.len())
            .map(|n| words[..n].join(" "))
            .find(|cand| {
                let lower = cand.to_ascii_lowercase();
                EXTS.iter().any(|ext| lower.ends_with(ext))
            })
            .unwrap_or_else(|| words[0].to_string())
    };
    let expanded = expand_env(raw.trim());
    (!expanded.is_empty()).then_some(expanded)
}

/// Like `command_executable`, but bare names such as `rundll32.exe` are looked
/// up in the Windows folders and on PATH. `None` if it can't be resolved.
pub fn resolve_executable(command: &str) -> Option<String> {
    let exe = command_executable(command)?;
    if exe.contains('\\') || exe.contains('/') {
        return Some(exe);
    }
    let name = if Path::new(&exe).extension().is_some() {
        exe
    } else {
        format!("{exe}.exe")
    };
    let windir = windows_dir();
    let mut dirs = vec![format!(r"{windir}\System32"), windir];
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path).map(|p| p.to_string_lossy().into_owned()));
    }
    dirs.into_iter()
        .map(|dir| Path::new(&dir).join(&name))
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
}

pub fn windows_dir() -> String {
    std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string())
}

#[cfg_attr(not(windows), allow(dead_code))]
fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix).then(|| &s[prefix.len()..])
}

/// Normalises the path forms Windows uses in service and driver entries:
/// `\??\C:\…`, `\SystemRoot\…`, `System32\…` and bare file names.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn normalize_system_path(path: &str) -> String {
    let windir = windows_dir();
    let p = path.trim();
    let p = strip_prefix_ci(p, r"\??\").unwrap_or(p);
    if let Some(rest) = strip_prefix_ci(p, r"\SystemRoot\") {
        return format!(r"{windir}\{rest}");
    }
    for dir in ["System32\\", "SysWOW64\\"] {
        if strip_prefix_ci(p, dir).is_some() {
            return format!(r"{windir}\{p}");
        }
    }
    if !p.contains('\\') && !p.contains('/') {
        return format!(r"{windir}\System32\{p}");
    }
    p.to_string()
}

/// `Some(true)` – the file is there, `Some(false)` – it is gone,
/// `None` – we could not tell (no path, or access was denied).
pub fn file_state(path: &str) -> Option<bool> {
    match std::fs::metadata(path) {
        Ok(_) => Some(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(false),
        Err(_) => None,
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn is_windows_path(path: &str) -> bool {
    let windir = windows_dir().to_ascii_lowercase();
    let p = path.to_ascii_lowercase();
    p.starts_with(&format!("{windir}\\"))
}

/// Runs `f` on every item using a few threads; results keep the input order.
pub fn parallel_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<R>>> = Mutex::new(items.iter().map(|_| None).collect());
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(2, 8);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(item) = items.get(i) else { break };
                let r = f(item);
                results.lock().unwrap()[i] = Some(r);
            });
        }
    });
    results.into_inner().unwrap().into_iter().flatten().collect()
}

/// Total size of all files below `path`, skipping anything unreadable.
pub fn dir_size(path: &Path) -> (u64, u64) {
    let mut bytes = 0u64;
    let mut files = 0u64;
    for entry in walkdir::WalkDir::new(path).follow_links(false).into_iter().flatten() {
        if entry.file_type().is_file() {
            if let Ok(meta) = entry.metadata() {
                bytes += meta.len();
                files += 1;
            }
        }
    }
    (bytes, files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_map_keeps_order() {
        let items: Vec<u32> = (0..100).collect();
        assert_eq!(
            parallel_map(&items, |n| n * 2),
            items.iter().map(|n| n * 2).collect::<Vec<_>>()
        );
    }

    #[test]
    fn expands_known_and_keeps_unknown_variables() {
        std::env::set_var("UWIPER_TEST_DIR", r"C:\Data");
        assert_eq!(expand_env(r"%UWIPER_TEST_DIR%\cache"), r"C:\Data\cache");
        assert_eq!(expand_env(r"%NOPE_NOT_SET%\x"), r"%NOPE_NOT_SET%\x");
        assert_eq!(expand_env("100% sure"), "100% sure");
    }

    #[test]
    fn finds_the_executable_in_command_lines() {
        assert_eq!(
            command_executable(r#""C:\Program Files\App\app.exe" --min"#).as_deref(),
            Some(r"C:\Program Files\App\app.exe")
        );
        assert_eq!(
            command_executable(r"C:\Program Files\App\app.exe /background").as_deref(),
            Some(r"C:\Program Files\App\app.exe")
        );
        assert_eq!(
            command_executable("rundll32 shell32.dll,Foo").as_deref(),
            Some("rundll32")
        );
        assert_eq!(
            command_executable(r"C:\Windows\system32\svchost.exe -k netsvcs").as_deref(),
            Some(r"C:\Windows\system32\svchost.exe")
        );
        assert_eq!(command_executable("   "), None);
    }

    #[test]
    fn normalizes_service_paths() {
        std::env::set_var("SystemRoot", r"C:\Windows");
        assert_eq!(
            normalize_system_path(r"\SystemRoot\System32\drivers\x.sys"),
            r"C:\Windows\System32\drivers\x.sys"
        );
        assert_eq!(
            normalize_system_path(r"System32\drivers\y.sys"),
            r"C:\Windows\System32\drivers\y.sys"
        );
        assert_eq!(normalize_system_path(r"\??\D:\Tools\svc.exe"), r"D:\Tools\svc.exe");
        assert_eq!(normalize_system_path("svchost.exe"), r"C:\Windows\System32\svchost.exe");
    }
}
