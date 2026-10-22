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

/// A program in System32, by full path, so nothing next to UWiper can stand in for it.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn system32(name: &str) -> std::path::PathBuf {
    Path::new(&windows_dir()).join("System32").join(name)
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
/// `None` – we could not tell (a relative path, access denied, or a drive or
/// network share that isn't there right now).
pub fn file_state(path: &str) -> Option<bool> {
    match std::fs::metadata(path) {
        Ok(_) => Some(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            volume_root(path).filter(|root| Path::new(root).exists()).map(|_| false)
        }
        Err(_) => None,
    }
}

/// `C:\` of `C:\x\y`, `\\server\share\` of a network path; `None` for a
/// relative path.
pub fn volume_root(path: &str) -> Option<String> {
    let p = path.trim().replace('/', "\\");
    let b = p.as_bytes();
    if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\' {
        return Some(p[..3].to_string());
    }
    if let Some(rest) = p.strip_prefix(r"\\") {
        let mut parts = rest.splitn(3, '\\');
        let (server, share) = (parts.next()?, parts.next()?);
        return (!server.is_empty() && !share.is_empty()).then(|| format!(r"\\{server}\{share}\"));
    }
    // Tests run on Linux.
    (cfg!(not(windows)) && path.starts_with('/')).then(|| "/".to_string())
}

/// Variables that point into the profile of whoever runs the program.
const PER_USER_VARS: [&str; 7] = [
    "%appdata%",
    "%localappdata%",
    "%userprofile%",
    "%temp%",
    "%tmp%",
    "%homepath%",
    "%onedrive%",
];

/// True when a command depends on which user runs it, so from this user's
/// view a missing file says nothing about another user's.
pub fn uses_per_user_vars(command: &str) -> bool {
    let c = command.to_lowercase();
    PER_USER_VARS.iter().any(|v| c.contains(v))
}

/// `file_state` for a program path, and `None` when the path still holds
/// a variable that couldn't be expanded.
pub fn program_state(path: &str) -> Option<bool> {
    if path.contains('%') {
        return None;
    }
    file_state(path)
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn is_windows_path(path: &str) -> bool {
    let windir = windows_dir().to_ascii_lowercase();
    let p = path.to_ascii_lowercase();
    p.starts_with(&format!("{windir}\\"))
}

/// Windows itself, also its parts outside the Windows folder: Defender,
/// Windows Media Player and the Update Health Tools in Program Files, and
/// Defender's platform in ProgramData.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn is_windows_component(path: &str) -> bool {
    let p = norm_path(path);
    let under = |var: &str, parts: &[&str]| {
        std::env::var(var).is_ok_and(|root| {
            let root = norm_path(&root);
            !root.is_empty() && parts.iter().any(|part| p.starts_with(&format!(r"{root}\{part}")))
        })
    };
    let program_files = ["windows", "microsoft update health tools"];
    is_windows_path(path)
        || under("ProgramFiles", &program_files)
        || under("ProgramW6432", &program_files)
        || under("ProgramFiles(x86)", &program_files)
        || under("ProgramData", &[r"microsoft\windows defender"])
}

/// What a program's version resource says about it: the company that made
/// it and its description, the two things Task Manager shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileInfo {
    pub company: Option<String>,
    pub description: Option<String>,
}

/// `FileInfo` of a file, read once per path.
pub fn file_info(path: &str) -> FileInfo {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    static CACHE: OnceLock<Mutex<HashMap<String, FileInfo>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = norm_path(path);
    if let Some(hit) = cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return hit;
    }
    let info = read_file_info(path);
    if let Ok(mut c) = cache.lock() {
        c.insert(key, info.clone());
    }
    info
}

#[cfg(not(windows))]
fn read_file_info(_path: &str) -> FileInfo {
    FileInfo::default()
}

#[cfg(windows)]
fn read_file_info(path: &str) -> FileInfo {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};

    let wide = |s: &str| {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<u16>>()
    };
    let file = wide(path);
    let size = unsafe { GetFileVersionInfoSizeW(file.as_ptr(), std::ptr::null_mut()) };
    if size == 0 {
        return FileInfo::default();
    }
    let mut data = vec![0u8; size as usize];
    if unsafe { GetFileVersionInfoW(file.as_ptr(), 0, size, data.as_mut_ptr().cast()) } == 0 {
        return FileInfo::default();
    }
    // The pointers point into `data`; `len` counts bytes for binary values and characters for text.
    let query = |sub: &str| {
        let sub = wide(sub);
        let mut ptr: *mut c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let ok = unsafe { VerQueryValueW(data.as_ptr().cast(), sub.as_ptr(), &mut ptr, &mut len) } != 0;
        (ok && !ptr.is_null() && len > 0).then_some((ptr as *const u16, len as usize))
    };
    let mut languages: Vec<String> = query(r"\VarFileInfo\Translation")
        .map(|(ptr, len)| {
            let pairs = unsafe { std::slice::from_raw_parts(ptr, len / 2) };
            pairs
                .chunks_exact(2)
                .map(|p| format!("{:04x}{:04x}", p[0], p[1]))
                .collect()
        })
        .unwrap_or_default();
    languages.extend(["040904b0", "040904e4", "000004b0"].map(String::from));
    let text = |name: &str| {
        languages.iter().find_map(|language| {
            let (ptr, len) = query(&format!(r"\StringFileInfo\{language}\{name}"))?;
            let units = unsafe { std::slice::from_raw_parts(ptr, len) };
            let value = String::from_utf16_lossy(units)
                .trim_end_matches('\0')
                .trim()
                .to_string();
            (!value.is_empty()).then_some(value)
        })
    };
    FileInfo {
        company: text("CompanyName"),
        description: text("FileDescription"),
    }
}

/// A part of Windows the way Task Manager tells: in a Windows place and made
/// by Microsoft. Drivers' helper programs from Realtek, Intel and others also
/// sit in System32, and they aren't Windows.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn is_windows_program(path: &str) -> bool {
    is_windows_component(path)
        && file_info(path)
            .company
            .is_none_or(|c| c.to_lowercase().starts_with("microsoft"))
}

/// Reads an indirect string such as `@%SystemRoot%\system32\audiosrv.dll,-204`
/// or `@{Package?ms-resource://…}` the way Windows shows it.
#[cfg(windows)]
pub fn indirect_string(source: &str) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::SHLoadIndirectString;

    let source: Vec<u16> = std::ffi::OsStr::new(source).encode_wide().chain(Some(0)).collect();
    let mut buf = vec![0u16; 512];
    let hr = unsafe { SHLoadIndirectString(source.as_ptr(), buf.as_mut_ptr(), buf.len() as u32, std::ptr::null()) };
    if hr < 0 {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let text = String::from_utf16_lossy(&buf[..len]).trim().to_string();
    (!text.is_empty() && !text.starts_with('@') && !text.starts_with("ms-resource:")).then_some(text)
}

/// Programs that only run another file: rundll32, the command shells and
/// the script hosts.
const HOSTS: [&str; 7] = ["rundll32", "cmd", "powershell", "pwsh", "wscript", "cscript", "mshta"];

/// The file a host program runs: the first argument that is a full path to a
/// library, script or program. `None` when `program` isn't a host or no
/// argument is such a path.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn hosted_file(program: &str, args: &str) -> Option<String> {
    const EXTS: [&str; 11] = [
        ".dll", ".exe", ".bat", ".cmd", ".ps1", ".vbs", ".vbe", ".js", ".jse", ".wsf", ".hta",
    ];
    if !is_host(program) {
        return None;
    }
    let mut tokens = Vec::new();
    let mut rest = args.trim();
    while !rest.is_empty() {
        let (token, after) = match rest.strip_prefix('"') {
            Some(quoted) => quoted.split_once('"').unwrap_or((quoted, "")),
            None => rest.split_once(char::is_whitespace).unwrap_or((rest, "")),
        };
        tokens.push(token);
        rest = after.trim_start();
    }
    tokens.into_iter().find_map(|token| {
        // rundll32 takes `file.dll,Entry`.
        let file = expand_env(token.split(',').next().unwrap_or_default().trim());
        let b = file.as_bytes();
        let absolute = (b.len() > 3 && b[1] == b':' && b[2] == b'\\') || file.starts_with(r"\\");
        let lower = file.to_ascii_lowercase();
        (absolute && EXTS.iter().any(|ext| lower.ends_with(ext))).then_some(file)
    })
}

pub fn is_host(program: &str) -> bool {
    let name = program
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(program)
        .to_ascii_lowercase();
    let stem = name.strip_suffix(".exe").unwrap_or(&name);
    HOSTS.contains(&stem)
}

/// Lowercase, backslashes, no trailing separator: for comparing Windows paths.
pub fn norm_path(path: &str) -> String {
    path.trim().replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

/// The long form of a path that exists (`C:\PROGRA~1` → `C:\Program Files`),
/// otherwise the path as given.
pub fn long_path(path: &str) -> String {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::GetLongPathNameW;

        let wide: Vec<u16> = std::ffi::OsStr::new(path).encode_wide().chain(Some(0)).collect();
        let mut buf = vec![0u16; 1024];
        let len = unsafe { GetLongPathNameW(wide.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
        if len > 0 && len < buf.len() {
            return String::from_utf16_lossy(&buf[..len]);
        }
    }
    path.to_string()
}

/// `norm_path` of the long form: what two spellings of one folder share.
pub fn canonical(path: &str) -> String {
    norm_path(&long_path(path.trim()))
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
    run_tool_with_env(name, args, &[])
}

#[cfg(not(windows))]
pub fn run_tool(_name: &str, _args: &[&str]) -> crate::error::Result<String> {
    Err(crate::error::Error::Unsupported)
}

/// `run_tool` with extra environment variables, for passing text that must
/// not be parsed as part of a command (PowerShell scripts, for one).
#[cfg(windows)]
pub fn run_tool_with_env(name: &str, args: &[&str], env: &[(&str, &str)]) -> crate::error::Result<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let output = std::process::Command::new(system32(name))
        .args(args)
        .envs(env.iter().copied())
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    let text = |bytes: &[u8]| console_text(bytes).trim().to_string();
    if output.status.success() {
        return Ok(text(&output.stdout));
    }
    let message = [text(&output.stderr), text(&output.stdout)]
        .into_iter()
        .find(|m| !m.is_empty())
        .unwrap_or_default();
    let lower = message.to_lowercase();
    if lower.contains("access is denied") || lower.contains("отказано в доступе") {
        return Err(crate::error::Error::NeedsAdmin);
    }
    Err(crate::error::Error::Other(if message.is_empty() {
        format!("{name} failed")
    } else {
        message
    }))
}

/// Console tools write in the OEM code page (866 on Russian Windows, for one),
/// not UTF-8.
#[cfg(windows)]
fn console_text(bytes: &[u8]) -> String {
    use windows_sys::Win32::Globalization::{MultiByteToWideChar, CP_OEMCP};
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }
    let Ok(len) = i32::try_from(bytes.len()) else {
        return String::from_utf8_lossy(bytes).into_owned();
    };
    unsafe {
        let wide_len = MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), len, std::ptr::null_mut(), 0);
        if wide_len <= 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        let mut wide = vec![0u16; wide_len as usize];
        MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), len, wide.as_mut_ptr(), wide_len);
        String::from_utf16_lossy(&wide)
    }
}

/// Text from a file Windows wrote: UTF-16 with or without a BOM, UTF-8, or an
/// ANSI code page (whose non-ASCII letters come out as replacement marks).
pub fn decode_text(bytes: &[u8]) -> String {
    let utf16 = |data: &[u8], le: bool| {
        let units: Vec<u16> = data
            .chunks_exact(2)
            .map(|c| {
                if le {
                    u16::from_le_bytes([c[0], c[1]])
                } else {
                    u16::from_be_bytes([c[0], c[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        // ASCII text saved as UTF-16 without a BOM: every second byte is zero.
        [_, 0, _, 0, ..] => utf16(bytes, true),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// A short, stable hash of some bytes (FNV-1a), for telling whether they changed.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn fnv(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        h ^= u64::from(*byte);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
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

/// The later of when a file was made and last written: an unpacked or copied
/// file keeps its old write time, but is made just now.
pub fn fresh_time(meta: &std::fs::Metadata) -> Option<std::time::SystemTime> {
    meta.created().ok().max(meta.modified().ok())
}

fn unix_secs(t: Option<std::time::SystemTime>) -> Option<u64> {
    t.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

/// What `dir_stats` finds in a file or folder.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct DirStats {
    pub bytes: u64,
    pub files: u64,
    pub dirs: u64,
    /// The newest change of anything, folders too (Unix time): what's shown.
    pub newest: Option<u64>,
    /// The newest file, made or written (Unix time). Unlike a folder's time,
    /// it doesn't move when something is deleted.
    pub newest_file: Option<u64>,
}

/// Size, counts and the newest changes of a file or of everything in a
/// folder. Links aren't followed.
pub fn dir_stats(path: &Path) -> DirStats {
    let mut stats = DirStats::default();
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return stats;
    };
    if !meta.is_dir() {
        let time = unix_secs(fresh_time(&meta));
        return DirStats {
            bytes: meta.len(),
            files: 1,
            newest: unix_secs(meta.modified().ok()),
            newest_file: time,
            ..stats
        };
    }
    stats.newest = unix_secs(meta.modified().ok());
    for entry in walkdir::WalkDir::new(path)
        .min_depth(1)
        .follow_links(false)
        .into_iter()
        .flatten()
    {
        if let Ok(m) = entry.metadata() {
            stats.newest = stats.newest.max(unix_secs(m.modified().ok()));
            if entry.file_type().is_dir() {
                stats.dirs += 1;
            } else {
                stats.bytes += m.len();
                stats.files += 1;
                stats.newest_file = stats.newest_file.max(unix_secs(fresh_time(&m)));
            }
        }
    }
    stats
}

/// Folders a low-rights user can't swap for a link: the profile's and
/// Windows' own. Anything below them is checked one by one.
fn trusted_roots() -> Vec<String> {
    [
        "TEMP",
        "LOCALAPPDATA",
        "APPDATA",
        "USERPROFILE",
        "ProgramData",
        "SystemRoot",
    ]
    .iter()
    .filter_map(|v| std::env::var(v).ok())
    .map(|p| norm_path(&p))
    .filter(|p| !p.is_empty())
    .collect()
}

/// Why a folder can't be worked in safely.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Blocked {
    /// It, or a folder on the way to it, is a link or junction.
    Link,
    Gone,
    Unreadable,
}

/// Holds a folder open so that, until it's dropped, the folder can't be
/// renamed, deleted or swapped for a link.
pub struct Pin {
    #[cfg(windows)]
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
impl Drop for Pin {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.handle) };
    }
}

/// Pins `dir` itself – not what a link points to, unless `follow`.
#[cfg(windows)]
fn pin_one(dir: &Path, follow: bool) -> std::result::Result<Pin, Blocked> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{GetLastError, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FileAttributeTagInfo, GetFileInformationByHandleEx, FILE_ATTRIBUTE_DIRECTORY,
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
        OPEN_EXISTING,
    };
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();
    let flags = FILE_FLAG_BACKUP_SEMANTICS | if follow { 0 } else { FILE_FLAG_OPEN_REPARSE_POINT };
    // Listing access makes the share mode count: without FILE_SHARE_DELETE
    // nobody else can rename or delete the folder while it's open.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            flags,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        // ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND
        return Err(match unsafe { GetLastError() } {
            2 | 3 => Blocked::Gone,
            _ => Blocked::Unreadable,
        });
    }
    let pin = Pin { handle };
    let mut info = FILE_ATTRIBUTE_TAG_INFO::default();
    let read = unsafe {
        GetFileInformationByHandleEx(
            pin.handle,
            FileAttributeTagInfo,
            (&mut info as *mut FILE_ATTRIBUTE_TAG_INFO).cast(),
            std::mem::size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
        )
    };
    if read == 0 {
        return Err(Blocked::Unreadable);
    }
    // Symbolic links and junctions stand for another place ("name
    // surrogates"); OneDrive's placeholders and the like are folders too.
    const NAME_SURROGATE: u32 = 0x2000_0000;
    if info.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 && info.ReparseTag & NAME_SURROGATE != 0 {
        return Err(Blocked::Link);
    }
    if info.FileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(Blocked::Unreadable);
    }
    Ok(pin)
}

#[cfg(not(windows))]
fn pin_one(dir: &Path, follow: bool) -> std::result::Result<Pin, Blocked> {
    let meta = if follow {
        std::fs::metadata(dir)
    } else {
        std::fs::symlink_metadata(dir)
    };
    match meta {
        Ok(m) if m.file_type().is_symlink() => Err(Blocked::Link),
        Ok(m) if m.is_dir() => Ok(Pin {}),
        Ok(_) => Err(Blocked::Unreadable),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Blocked::Gone),
        Err(_) => Err(Blocked::Unreadable),
    }
}

/// Pins a folder that must not be a link.
pub fn pin_dir(dir: &Path) -> std::result::Result<Pin, Blocked> {
    pin_one(dir, false)
}

/// Pins every folder on the way to `path`, from the closest trusted root (or
/// the drive) down to its parent, so none of them can be swapped for a link
/// while UWiper works there. Above a trusted root links are fine: a TEMP
/// moved to another drive is the user's choice.
pub fn pin_parents(path: &Path) -> std::result::Result<Vec<Pin>, Blocked> {
    let roots = trusted_roots();
    let mut chain: Vec<&Path> = Vec::new();
    let mut top: Option<&Path> = None;
    let mut current = path.parent();
    while let Some(p) = current {
        if roots.contains(&norm_path(&p.to_string_lossy())) || p.parent().is_none() {
            top = Some(p);
            break;
        }
        chain.push(p);
        current = p.parent();
    }
    let mut pins = Vec::with_capacity(chain.len() + 1);
    if let Some(top) = top {
        pins.push(pin_one(top, true)?);
    }
    for dir in chain.into_iter().rev() {
        pins.push(pin_dir(dir)?);
    }
    Ok(pins)
}

/// Flushes a written file to the disk, so it outlasts a power cut that
/// comes right after. A read-only copy is made writable for that and put
/// back.
pub fn flush_file(path: &Path) -> std::io::Result<()> {
    let open = || std::fs::OpenOptions::new().write(true).open(path);
    match open() {
        Ok(file) => file.sync_all(),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            let perms = std::fs::metadata(path)?.permissions();
            if !perms.readonly() {
                return Err(e);
            }
            let mut writable = perms.clone();
            #[allow(clippy::permissions_set_readonly_false)]
            writable.set_readonly(false);
            std::fs::set_permissions(path, writable)?;
            let flushed = open().and_then(|file| file.sync_all());
            let back = std::fs::set_permissions(path, perms);
            flushed.and(back)
        }
        Err(e) => Err(e),
    }
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
    fn flushes_a_read_only_copy_and_keeps_it_read_only() {
        let file = std::env::temp_dir().join(format!("uwiper-ro-{}.txt", std::process::id()));
        std::fs::write(&file, b"keep").unwrap();
        let mut perms = std::fs::metadata(&file).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&file, perms.clone()).unwrap();
        flush_file(&file).unwrap();
        assert!(std::fs::metadata(&file).unwrap().permissions().readonly());
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        std::fs::set_permissions(&file, perms).unwrap();
        std::fs::remove_file(&file).unwrap();
    }

    #[test]
    fn a_missing_drive_says_nothing_about_a_file() {
        assert_eq!(volume_root(r"C:\Tools\a.exe").as_deref(), Some(r"C:\"));
        assert_eq!(volume_root(r"\\nas\share\a.exe").as_deref(), Some(r"\\nas\share\"));
        assert_eq!(volume_root(r"bin\a.exe"), None);
        // No Q: drive here: an unplugged USB stick, say.
        assert_eq!(file_state(r"Q:\Tools\gone.exe"), None);
        assert_eq!(file_state(r"bin\gone.exe"), None, "a relative path can't be told");
        let gone = std::env::temp_dir().join(format!("uwiper-surely-gone-{}.exe", std::process::id()));
        assert_eq!(file_state(&gone.to_string_lossy()), Some(false));
    }

    #[test]
    fn deleting_a_file_leaves_the_newest_file_time() {
        let dir = std::env::temp_dir().join(format!("uwiper-stats-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.txt"), b"aa").unwrap();
        std::fs::write(dir.join("sub/b.txt"), b"bbb").unwrap();
        let before = dir_stats(&dir);
        assert_eq!((before.files, before.bytes, before.dirs), (2, 5, 1));
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::remove_file(dir.join("sub/b.txt")).unwrap();
        let after = dir_stats(&dir);
        assert_eq!((after.files, after.bytes), (1, 2));
        assert!(
            after.newest_file <= before.newest_file,
            "a folder's time moves, a file's doesn't"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn pins_refuse_links_on_the_way() {
        let dir = std::env::temp_dir().join(format!("uwiper-pins-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("real/inner")).unwrap();
        std::os::unix::fs::symlink(dir.join("real"), dir.join("link")).unwrap();
        assert!(pin_parents(&dir.join("real/inner/x")).is_ok());
        assert_eq!(pin_parents(&dir.join("link/inner/x")).err(), Some(Blocked::Link));
        assert_eq!(pin_dir(&dir.join("link")).err(), Some(Blocked::Link));
        assert_eq!(pin_dir(&dir.join("nothing")).err(), Some(Blocked::Gone));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn finds_what_hosts_run() {
        let rundll = r"C:\Windows\System32\rundll32.exe";
        assert_eq!(
            hosted_file(rundll, r#""C:\Program Files\Vendor\tray.dll",Start /quiet"#).as_deref(),
            Some(r"C:\Program Files\Vendor\tray.dll")
        );
        assert_eq!(
            hosted_file(r"C:\Windows\System32\cmd.exe", r#"/c start "" C:\Tools\sync.bat"#).as_deref(),
            Some(r"C:\Tools\sync.bat")
        );
        assert_eq!(
            hosted_file(
                "powershell.exe",
                r"-ExecutionPolicy Bypass -File \\nas\scripts\login.ps1"
            )
            .as_deref(),
            Some(r"\\nas\scripts\login.ps1")
        );
        assert_eq!(hosted_file(rundll, "shell32.dll,Control_RunDLL"), None);
        assert_eq!(hosted_file(r"C:\App\app.exe", r"C:\App\plugin.dll"), None);
        assert!(is_host("RUNDLL32.EXE") && is_host(r"C:\x\pwsh.exe") && !is_host("app.exe"));
    }

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
