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

/// The program part of a command line, as typed: the quoted part, or the
/// shortest run of words that ends in an executable extension.
fn program_token(command: &str) -> Option<&str> {
    let s = command.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(rest) = s.strip_prefix('"') {
        let end = rest.find('"').unwrap_or(rest.len());
        return Some(&s[..(end + 2).min(s.len())]);
    }
    // Unquoted paths may contain spaces: take the shortest run of words
    // that ends in an executable extension, else just the first word.
    const EXTS: [&str; 6] = [".exe", ".com", ".bat", ".cmd", ".dll", ".sys"];
    let mut end = 0;
    for word in s.split(' ') {
        end += word.len();
        if EXTS.iter().any(|ext| s[..end].to_ascii_lowercase().ends_with(ext)) {
            return Some(&s[..end]);
        }
        end += 1;
    }
    Some(s.split(' ').next().unwrap_or(s))
}

/// Pulls the executable out of a command line such as
/// `"C:\Program Files\App\app.exe" --minimized` or `C:\Tools\app.exe -x`.
pub fn command_executable(command: &str) -> Option<String> {
    let expanded = expand_env(program_token(command)?.trim_matches('"').trim());
    (!expanded.is_empty()).then_some(expanded)
}

/// Splits a command line into the program and its arguments, the arguments
/// kept exactly as written.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn split_command(command: &str) -> Option<(String, String)> {
    let token = program_token(command)?;
    let args = command.trim()[token.len()..].trim().to_string();
    Some((command_executable(command)?, args))
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

/// Lowercase, backslashes, no trailing separator: for comparing Windows paths.
pub fn norm_path(path: &str) -> String {
    path.trim().replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

/// True when `path` is `root` or something inside it.
pub fn is_within(path: &str, root: &str) -> bool {
    let (p, r) = (norm_path(path), norm_path(root));
    !r.is_empty() && (p == r || p.starts_with(&format!("{r}\\")))
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// `YYYY-MM-DD` in UTC for a Unix time.
pub fn utc_date(secs: u64) -> String {
    // Howard Hinnant's days-to-civil algorithm.
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Runs a tool from System32 (reg, sc, schtasks, powershell) without a
/// console window. A non-zero exit becomes an error carrying its message.
#[cfg(windows)]
pub fn run_tool(name: &str, args: &[&str]) -> crate::error::Result<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let program = Path::new(&windows_dir()).join("System32").join(name);
    let output = std::process::Command::new(program)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).trim().to_string();
    if output.status.success() {
        return Ok(text(&output.stdout));
    }
    let message = [text(&output.stderr), text(&output.stdout)]
        .into_iter()
        .find(|m| !m.is_empty())
        .unwrap_or_default();
    if message.to_lowercase().contains("access is denied") {
        return Err(crate::error::Error::NeedsAdmin);
    }
    Err(crate::error::Error::Other(if message.is_empty() {
        format!("{name} failed")
    } else {
        message
    }))
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
    fn splits_command_lines() {
        assert_eq!(
            split_command(r#""C:\Program Files\App\unins000.exe" /SILENT"#),
            Some((r"C:\Program Files\App\unins000.exe".into(), "/SILENT".into()))
        );
        assert_eq!(
            split_command(r"C:\Program Files\App\uninstall.exe --force now"),
            Some((r"C:\Program Files\App\uninstall.exe".into(), "--force now".into()))
        );
        assert_eq!(
            split_command("MsiExec.exe /X{1234}"),
            Some(("MsiExec.exe".into(), "/X{1234}".into()))
        );
        assert_eq!(
            split_command(r#""C:\App\u.exe""#),
            Some((r"C:\App\u.exe".into(), String::new()))
        );
    }

    #[test]
    fn compares_windows_paths() {
        assert!(is_within(r"C:\Program Files\App\bin", r"c:\program files\app\"));
        assert!(is_within(r"C:\Program Files\App", r"C:\Program Files\App"));
        assert!(!is_within(r"C:\Program Files\App2", r"C:\Program Files\App"));
        assert!(!is_within(r"C:\x", ""));
    }

    #[test]
    fn formats_utc_dates() {
        assert_eq!(utc_date(0), "1970-01-01");
        assert_eq!(utc_date(1_792_540_800), "2026-10-21");
        assert_eq!(utc_date(951_782_400), "2000-02-29");
    }

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
