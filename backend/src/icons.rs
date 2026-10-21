//! App icons: pulled out of the app's exe (or .ico) and handed to the UI as
//! PNG data URLs.
#![cfg_attr(not(windows), allow(dead_code))]

use std::path::{Path, PathBuf};

use crate::util::expand_env;

/// Pixels per side. Big enough to stay sharp on the 56 px detail tile at 150 %.
const SIZE: u32 = 96;

/// Splits a `DisplayIcon` value such as `"C:\App\app.exe",0` or
/// `%SystemRoot%\system32\x.dll,-101` into the file and the icon index.
pub fn parse_spec(spec: &str) -> Option<(PathBuf, i32)> {
    let spec = spec.trim();
    let (file, index) = match spec.rsplit_once(',') {
        Some((file, index)) if index.trim().parse::<i32>().is_ok() => (file, index.trim().parse().unwrap_or(0)),
        _ => (spec, 0),
    };
    let file = expand_env(file.trim().trim_matches('"').trim());
    (!file.is_empty()).then(|| (PathBuf::from(file), index))
}

/// The file name without extension, lowercased. Splits on `\\` itself so it
/// also works on Windows paths in tests on other systems.
pub fn stem(file: &Path) -> String {
    let path = file.to_string_lossy();
    let name = path.rsplit(['\\', '/']).next().unwrap_or_default();
    name.rsplit_once('.').map_or(name, |(stem, _)| stem).to_lowercase()
}

fn stem_starts_with(file: &Path, words: &[&str]) -> bool {
    let stem = stem(file);
    words.iter().any(|word| stem.starts_with(word))
}

/// Uninstallers carry a generic icon, not the app's own.
pub fn is_uninstaller(file: &Path) -> bool {
    stem_starts_with(file, &["unins", "uninst", "msiexec"])
}

/// Programs next to the app's exe that aren't the app itself.
pub fn is_helper_exe(file: &Path) -> bool {
    is_uninstaller(file)
        || stem_starts_with(
            file,
            &["setup", "install", "update", "crash", "helper", "elevat", "vc_redist"],
        )
}

/// Reads the icon and encodes it as `data:image/png;base64,…`.
pub fn data_url(file: &Path, index: i32) -> Option<String> {
    use base64::Engine;

    let (width, height, rgba) = imp::extract(file, index, SIZE)?;
    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&rgba).ok()?;
    }
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png_bytes)
    ))
}

#[cfg(not(windows))]
mod imp {
    use std::path::Path;

    pub fn extract(_file: &Path, _index: i32, _size: u32) -> Option<(u32, u32, Vec<u8>)> {
        None
    }
}

#[cfg(windows)]
mod imp {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::ptr::null_mut;

    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
        BI_RGB, DIB_RGB_COLORS, HBITMAP,
    };
    use windows_sys::Win32::UI::Shell::SHDefExtractIconW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};

    /// Returns width, height and RGBA pixels.
    pub fn extract(file: &Path, index: i32, size: u32) -> Option<(u32, u32, Vec<u8>)> {
        let path: Vec<u16> = file.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut icon: HICON = null_mut();
        // Low word: the size for the large icon; the small one isn't requested.
        let found = unsafe { SHDefExtractIconW(path.as_ptr(), index, 0, &mut icon, null_mut(), size) };
        if found != 0 || icon.is_null() {
            return None;
        }
        let pixels = pixels(icon);
        unsafe { DestroyIcon(icon) };
        pixels
    }

    fn pixels(icon: HICON) -> Option<(u32, u32, Vec<u8>)> {
        let mut info: ICONINFO = unsafe { std::mem::zeroed() };
        if unsafe { GetIconInfo(icon, &mut info) } == 0 {
            return None;
        }
        let result = (|| {
            if info.hbmColor.is_null() {
                return None;
            }
            let mut bitmap: BITMAP = unsafe { std::mem::zeroed() };
            let len = std::mem::size_of::<BITMAP>() as i32;
            if unsafe { GetObjectW(info.hbmColor, len, (&mut bitmap as *mut BITMAP).cast()) } == 0 {
                return None;
            }
            let (width, height) = (bitmap.bmWidth, bitmap.bmHeight);
            let mut color = bits(info.hbmColor, width, height)?;
            // Old icons have no alpha channel; their mask says what's transparent.
            if color.chunks_exact(4).all(|px| px[3] == 0) {
                let mask = bits(info.hbmMask, width, height)?;
                for (px, m) in color.chunks_exact_mut(4).zip(mask.chunks_exact(4)) {
                    px[3] = if m[0] == 0 { 255 } else { 0 };
                }
            }
            for px in color.chunks_exact_mut(4) {
                px.swap(0, 2); // BGRA → RGBA
            }
            Some((width as u32, height as u32, color))
        })();
        unsafe {
            DeleteObject(info.hbmColor);
            DeleteObject(info.hbmMask);
        }
        result
    }

    /// The bitmap as top-down 32-bit BGRA rows.
    fn bits(bitmap: HBITMAP, width: i32, height: i32) -> Option<Vec<u8>> {
        if width <= 0 || height <= 0 || width > 512 || height > 512 {
            return None;
        }
        let mut info: BITMAPINFO = unsafe { std::mem::zeroed() };
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..unsafe { std::mem::zeroed() }
        };
        let mut buf = vec![0u8; (width * height * 4) as usize];
        let lines = unsafe {
            let dc = CreateCompatibleDC(null_mut());
            let lines = GetDIBits(
                dc,
                bitmap,
                0,
                height as u32,
                buf.as_mut_ptr().cast(),
                &mut info,
                DIB_RGB_COLORS,
            );
            DeleteDC(dc);
            lines
        };
        (lines == height).then_some(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_display_icon_values() {
        assert_eq!(
            parse_spec(r#""C:\App\app.exe",0"#),
            Some((PathBuf::from(r"C:\App\app.exe"), 0))
        );
        assert_eq!(
            parse_spec(r"C:\App\app.exe,-101"),
            Some((PathBuf::from(r"C:\App\app.exe"), -101))
        );
        assert_eq!(
            parse_spec(r"C:\App\icon.ico"),
            Some((PathBuf::from(r"C:\App\icon.ico"), 0))
        );
        assert_eq!(
            parse_spec(r"C:\My, App\app.exe"),
            Some((PathBuf::from(r"C:\My, App\app.exe"), 0))
        );
        assert_eq!(parse_spec(" "), None);
    }

    #[test]
    fn spots_uninstallers_and_helpers() {
        assert!(is_uninstaller(Path::new(r"C:\App\unins000.exe")));
        assert!(is_uninstaller(Path::new(r"C:\Windows\System32\MsiExec.exe")));
        assert!(!is_uninstaller(Path::new(r"C:\App\Update.exe")));
        assert!(is_helper_exe(Path::new(r"C:\App\Update.exe")));
        assert!(!is_helper_exe(Path::new(r"C:\App\Ravenswatch.exe")));
    }
}
