//! Application icon extraction.
//!
//! On Windows the icon is pulled straight out of the executable with
//! `SHGetFileInfoW`, drawn into a 32-bit top-down DIB and encoded as PNG.
//! Everywhere else this is a no-op and the UI falls back to a coloured letter
//! avatar, so no platform-specific work is required for a correct build.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Extraction is cheap but not free, and the picker asks for the same handful
/// of apps repeatedly, so results are memoised for the process lifetime.
fn cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// PNG icon for an executable as a `data:` URI, or `None` when unavailable.
pub fn icon_data_uri(app_name: &str, exe_path: &str) -> Option<String> {
    if exe_path.is_empty() {
        return None;
    }

    if let Ok(guard) = cache().lock() {
        if let Some(cached) = guard.get(app_name) {
            return cached.clone();
        }
    }

    let result = extract_png(exe_path, ICON_SIZE).map(|png| to_data_uri(&png));

    if let Ok(mut guard) = cache().lock() {
        guard.insert(app_name.to_string(), result.clone());
    }
    result
}

fn to_data_uri(png: &[u8]) -> String {
    use base64::Engine;
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    )
}

/// Size requested from the shell; large enough for a crisp list icon on hidpi.
const ICON_SIZE: u32 = 32;

#[cfg(not(target_os = "windows"))]
fn extract_png(_exe_path: &str, _size: u32) -> Option<Vec<u8>> {
    None
}

#[cfg(target_os = "windows")]
fn extract_png(exe_path: &str, size: u32) -> Option<Vec<u8>> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;

    let wide: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut info = SHFILEINFOW::default();

    let ok = unsafe {
        SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        )
    };
    if ok == 0 || info.hIcon.0.is_null() {
        return None;
    }

    let pixels = unsafe { draw_icon(hicon_handle(&info), size) };
    unsafe {
        let _ = DestroyIcon(hicon_handle(&info));
    }

    let (rgba, width, height) = pixels?;
    encode_png(&rgba, width, height)
}

#[cfg(target_os = "windows")]
fn hicon_handle(info: &windows::Win32::UI::Shell::SHFILEINFOW) -> windows::Win32::UI::WindowsAndMessaging::HICON {
    info.hIcon
}

/// Render an `HICON` into a top-down 32-bit DIB and return straight-alpha RGBA.
#[cfg(target_os = "windows")]
unsafe fn draw_icon(
    hicon: windows::Win32::UI::WindowsAndMessaging::HICON,
    size: u32,
) -> Option<(Vec<u8>, u32, u32)> {
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows::Win32::UI::WindowsAndMessaging::{DrawIconEx, DI_NORMAL};

    let hdc = CreateCompatibleDC(None);
    if hdc.0.is_null() {
        return None;
    }

    let mut bmi = BITMAPINFO::default();
    bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = size as i32;
    // Negative height asks for a top-down bitmap, matching PNG row order.
    bmi.bmiHeader.biHeight = -(size as i32);
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB.0;

    let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
    let hbmp = match CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
        Ok(h) if !h.0.is_null() && !bits.is_null() => h,
        _ => {
            let _ = DeleteDC(hdc);
            return None;
        }
    };

    let previous = SelectObject(hdc, hbmp);
    let drawn = DrawIconEx(hdc, 0, 0, hicon, size as i32, size as i32, 0, None, DI_NORMAL).is_ok();
    SelectObject(hdc, previous);
    let _ = DeleteObject(hbmp);
    let _ = DeleteDC(hdc);

    if !drawn {
        return None;
    }

    let count = (size as usize) * (size as usize);
    let raw = std::slice::from_raw_parts(bits as *const u8, count * 4);
    Some((bgra_to_rgba(raw), size, size))
}

/// DIB rows are BGRA. GDIs icon path produces premultiplied alpha, but not
/// every icon goes through that path, so detect which we got before
/// converting — guessing wrong either darkens edges or leaves halos.
#[cfg(target_os = "windows")]
fn bgra_to_rgba(raw: &[u8]) -> Vec<u8> {
    let premultiplied = raw.chunks_exact(4).any(|p| p[3] != 0)
        && raw
            .chunks_exact(4)
            .all(|p| p[0].max(p[1]).max(p[2]) <= p[3]);

    let mut out = vec![0u8; raw.len()];
    for (src, dst) in raw.chunks_exact(4).zip(out.chunks_exact_mut(4)) {
        let (b, g, r, a) = (src[0], src[1], src[2], src[3]);
        let (r, g, b) = if a == 0 {
            (0, 0, 0)
        } else if premultiplied {
            (
                unpremultiply(r, a),
                unpremultiply(g, a),
                unpremultiply(b, a),
            )
        } else {
            (r, g, b)
        };
        dst[0] = r;
        dst[1] = g;
        dst[2] = b;
        dst[3] = a;
    }
    out
}

#[cfg(target_os = "windows")]
fn unpremultiply(channel: u8, alpha: u8) -> u8 {
    let value = (channel as u32 * 255 + alpha as u32 / 2) / alpha as u32;
    value.min(255) as u8
}

#[cfg(any(target_os = "windows", test))]
fn encode_png(rgba: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
        writer.finish().ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_encoding_round_trips_through_a_decoder() {
        let rgba: Vec<u8> = vec![255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, 9, 9, 9, 255];
        let png = encode_png(&rgba, 2, 2).expect("encode");
        assert_eq!(&png[1..4], b"PNG");

        let decoder = png::Decoder::new(png.as_slice());
        let mut reader = decoder.read_info().expect("decode header");
        let mut buf = vec![0u8; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).expect("decode frame");
        assert_eq!(info.width, 2);
        assert_eq!(info.height, 2);
        assert_eq!(&buf[..info.buffer_size()], &rgba[..]);
    }

    #[test]
    fn missing_executable_path_yields_no_icon() {
        assert!(icon_data_uri("brave", "").is_none());
    }
}
