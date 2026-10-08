//! The Roblox player's icon: the picture on the Roblox window's title bar
//! and taskbar button, swapped for Pious's, a bootstrapper's, one of
//! Roblox's icons through the years, or your own picture.
//!
//! The executable isn't touched: Pious tells each Roblox window to use the
//! icon once it opens (like it does with the window title).

use std::path::Path;

use image::{ImageFormat, RgbaImage};

/// Where the presets other than Pious's come from (Fishstrap's open-source
/// resources, which Bloxstrap's icon choices also come from).
const SOURCE: &str = "https://raw.githubusercontent.com/fishstrap/fishstrap/main/Bloxstrap/Resources";

/// The presets: ID, name, file in [`SOURCE`] ("" = Pious's own).
pub const PRESETS: [(&str, &str, &str); 11] = [
    ("pious", "Pious", ""),
    // In Fishstrap's files, IconBloxstrap.ico is Fishstrap's own fish and
    // IconFishstrap.ico is Bloxstrap's tiles.
    ("bloxstrap", "Bloxstrap", "IconFishstrap.ico"),
    ("bloxstrap_classic", "Bloxstrap (classic)", "IconBloxstrapClassic.ico"),
    ("fishstrap", "Fishstrap", "IconBloxstrap.ico"),
    ("2008", "Roblox 2008", "Icon2008.ico"),
    ("2011", "Roblox 2011", "Icon2011.ico"),
    ("early2015", "Roblox early 2015", "IconEarly2015.ico"),
    ("late2015", "Roblox late 2015", "IconLate2015.ico"),
    ("2017", "Roblox 2017", "Icon2017.ico"),
    ("2019", "Roblox 2019", "Icon2019.ico"),
    ("2022", "Roblox 2022", "Icon2022.ico"),
];

const PIOUS: &[u8] = include_bytes!("../../icons/icon.png");

/// The picture for a preset, or a file for "custom".
pub async fn load(choice: &str, file: Option<&Path>, cache: &Path) -> Result<RgbaImage, String> {
    let bytes = if choice == "custom" {
        let file = file.ok_or("Pick a picture for the Roblox icon.")?;
        tokio::fs::read(file).await.map_err(|e| format!("Couldn't read {} ({e}).", file.display()))?
    } else {
        let (_, _, name) = PRESETS.iter().find(|(id, ..)| *id == choice).ok_or("That icon isn't one Pious knows.")?;
        if name.is_empty() {
            PIOUS.to_vec()
        } else {
            super::fonts::cached_download(&format!("{SOURCE}/{name}"), cache, &format!("icon-{name}")).await?
        }
    };
    let image = image::load_from_memory(&bytes).map_err(|e| format!("That picture can't be used as an icon ({e})."))?;
    Ok(image.to_rgba8())
}

/// A small PNG of the icon as a data: URL, for showing it in Pious.
pub fn preview(image: &RgbaImage) -> String {
    use base64::Engine;
    let small = image::imageops::resize(image, 64, 64, image::imageops::FilterType::Lanczos3);
    let mut out = std::io::Cursor::new(Vec::new());
    let _ = small.write_to(&mut out, ImageFormat::Png);
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(out.into_inner()))
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use windows_sys::Win32::Graphics::Gdi::{BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS, DeleteObject, HBITMAP};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateIconIndirect, HICON, ICON_BIG, ICON_SMALL, ICONINFO, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_GETICON, WM_SETICON,
    };

    /// Icons made so far, by key and size (kept for the life of Pious; a
    /// window shows the icon only while it exists).
    static ICONS: Mutex<Option<HashMap<(String, u32), isize>>> = Mutex::new(None);

    fn hicon(image: &RgbaImage, size: u32) -> Option<HICON> {
        let image = image::imageops::resize(image, size, size, image::imageops::FilterType::Lanczos3);
        unsafe {
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size as i32,
                // Negative: top-down rows.
                biHeight: -(size as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let color: HBITMAP = CreateDIBSection(std::ptr::null_mut(), &info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
            if color.is_null() || bits.is_null() {
                return None;
            }
            // Windows wants BGRA, premultiplied is not needed for icons.
            let pixels = std::slice::from_raw_parts_mut(bits as *mut u8, (size * size * 4) as usize);
            for (out, p) in pixels.chunks_exact_mut(4).zip(image.pixels()) {
                out.copy_from_slice(&[p.0[2], p.0[1], p.0[0], p.0[3]]);
            }
            let mask = CreateBitmap(size as i32, size as i32, 1, 1, std::ptr::null());
            let icon = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
            let handle = CreateIconIndirect(&icon);
            DeleteObject(color);
            DeleteObject(mask);
            (!handle.is_null()).then_some(handle)
        }
    }

    fn cached(key: &str, image: &RgbaImage, size: u32) -> Option<HICON> {
        let mut icons = ICONS.lock().ok()?;
        let map = icons.get_or_insert_with(HashMap::new);
        if let Some(&h) = map.get(&(key.to_owned(), size)) {
            return Some(h as HICON);
        }
        let h = hicon(image, size)?;
        map.insert((key.to_owned(), size), h as isize);
        Some(h)
    }

    /// Gives a window the icon (`key` names the picture, so it's only made
    /// once). Does nothing if it already has it.
    pub fn set(window: isize, key: &str, image: &RgbaImage) {
        let (Some(big), Some(small)) = (cached(key, image, 32), cached(key, image, 16)) else { return };
        unsafe {
            let mut current = 0usize;
            let asked = SendMessageTimeoutW(window as _, WM_GETICON, ICON_BIG as usize, 0, SMTO_ABORTIFHUNG, 300, &mut current);
            if asked != 0 && current == big as usize {
                return;
            }
            SendMessageTimeoutW(window as _, WM_SETICON, ICON_BIG as usize, big as isize, SMTO_ABORTIFHUNG, 300, std::ptr::null_mut());
            SendMessageTimeoutW(window as _, WM_SETICON, ICON_SMALL as usize, small as isize, SMTO_ABORTIFHUNG, 300, std::ptr::null_mut());
        }
    }

    /// Takes Pious's icon off a window, so it shows Roblox's own again.
    pub fn reset(window: isize) {
        unsafe {
            SendMessageTimeoutW(window as _, WM_SETICON, ICON_BIG as usize, 0, SMTO_ABORTIFHUNG, 300, std::ptr::null_mut());
            SendMessageTimeoutW(window as _, WM_SETICON, ICON_SMALL as usize, 0, SMTO_ABORTIFHUNG, 300, std::ptr::null_mut());
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn set(_window: isize, _key: &str, _image: &RgbaImage) {}
    pub fn reset(_window: isize) {}
}

pub use imp::{reset, set};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pious_icon_loads() {
        let image = image::load_from_memory(PIOUS).unwrap();
        assert!(image.width() >= 32);
        assert!(preview(&image.to_rgba8()).starts_with("data:image/png;base64,"));
    }
}
