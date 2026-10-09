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

    pub fn hicon(image: &RgbaImage, size: u32) -> Option<HICON> {
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

    /// The icon sizes Windows wants for a window on its screen (bigger on
    /// high-DPI screens; a 32-pixel icon there looked blurry or was ignored).
    fn sizes(window: isize) -> (u32, u32) {
        use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
        use windows_sys::Win32::UI::WindowsAndMessaging::{SM_CXICON, SM_CXSMICON};
        unsafe {
            let dpi = GetDpiForWindow(window as _).max(96);
            (GetSystemMetricsForDpi(SM_CXICON, dpi).max(32) as u32, GetSystemMetricsForDpi(SM_CXSMICON, dpi).max(16) as u32)
        }
    }

    /// Gives a window the icon (`key` names the picture, so it's only made
    /// once): its title bar and Alt+Tab (the window's own icon), and its
    /// taskbar button. Windows draws a taskbar button from the app the
    /// window belongs to, not the window's icon, so the window also gets an
    /// app ID of its own with this picture as that app's icon. Does nothing
    /// if it already has it.
    pub fn set(window: isize, key: &str, image: &RgbaImage) {
        let (big_size, small_size) = sizes(window);
        let (Some(big), Some(small)) = (cached(key, image, big_size), cached(key, image, small_size)) else { return };
        unsafe {
            let mut current = 0usize;
            let asked = SendMessageTimeoutW(window as _, WM_GETICON, ICON_BIG as usize, 0, SMTO_ABORTIFHUNG, 300, &mut current);
            if asked != 0 && current == big as usize {
                return;
            }
            SendMessageTimeoutW(window as _, WM_SETICON, ICON_BIG as usize, big as isize, SMTO_ABORTIFHUNG, 300, std::ptr::null_mut());
            SendMessageTimeoutW(window as _, WM_SETICON, ICON_SMALL as usize, small as isize, SMTO_ABORTIFHUNG, 300, std::ptr::null_mut());
        }
        if let Some(ico) = ico_file(key, image) {
            taskbar::set(window, Some((&ico, key)));
        }
    }

    /// Takes Pious's icon off a window, so it shows Roblox's own again.
    pub fn reset(window: isize) {
        unsafe {
            SendMessageTimeoutW(window as _, WM_SETICON, ICON_BIG as usize, 0, SMTO_ABORTIFHUNG, 300, std::ptr::null_mut());
            SendMessageTimeoutW(window as _, WM_SETICON, ICON_SMALL as usize, 0, SMTO_ABORTIFHUNG, 300, std::ptr::null_mut());
        }
        taskbar::set(window, None);
    }

    /// The picture as a .ico file (the taskbar needs a file to read the
    /// icon from), in every size Windows uses.
    fn ico_file(key: &str, image: &RgbaImage) -> Option<std::path::PathBuf> {
        use image::codecs::ico::{IcoEncoder, IcoFrame};
        let name: String = key.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).take(60).collect();
        let hash = key.bytes().fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64));
        let path = crate::core::store::data_dir().join("cache").join("icons").join(format!("{name}-{hash:x}.ico"));
        if path.is_file() {
            return Some(path);
        }
        let frames: Vec<IcoFrame> = [16u32, 24, 32, 48, 64, 128, 256]
            .iter()
            .filter_map(|&s| {
                let small = image::imageops::resize(image, s, s, image::imageops::FilterType::Lanczos3);
                IcoFrame::as_png(small.as_raw(), s, s, image::ExtendedColorType::Rgba8).ok()
            })
            .collect();
        std::fs::create_dir_all(path.parent()?).ok()?;
        let file = std::fs::File::create(&path).ok()?;
        IcoEncoder::new(file).encode_images(&frames).ok()?;
        Some(path)
    }

    /// A window's taskbar identity: its own app ID with an icon, or back to
    /// Roblox's (`None`).
    mod taskbar {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Storage::EnhancedStorage::{
            PKEY_AppUserModel_ID, PKEY_AppUserModel_RelaunchCommand, PKEY_AppUserModel_RelaunchDisplayNameResource,
            PKEY_AppUserModel_RelaunchIconResource,
        };
        use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
        use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow};
        use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;

        pub fn set(window: isize, icon: Option<(&std::path::Path, &str)>) {
            unsafe {
                // This thread may not have COM yet; undone only if this started it.
                let started = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
                if let Ok(store) = SHGetPropertyStoreForWindow::<IPropertyStore>(HWND(window as _)) {
                    match icon {
                        Some((file, key)) => {
                            let id: String = format!("Pious.Roblox.{}", key.chars().filter(|c| c.is_ascii_alphanumeric()).take(40).collect::<String>());
                            let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
                            // The relaunch values go first: the ID makes Windows read them.
                            let _ = store.SetValue(&PKEY_AppUserModel_RelaunchIconResource, &PROPVARIANT::from(format!("{},0", file.display()).as_str()));
                            let _ = store.SetValue(&PKEY_AppUserModel_RelaunchDisplayNameResource, &PROPVARIANT::from("Roblox"));
                            let _ = store.SetValue(&PKEY_AppUserModel_RelaunchCommand, &PROPVARIANT::from(format!("\"{exe}\"").as_str()));
                            let _ = store.SetValue(&PKEY_AppUserModel_ID, &PROPVARIANT::from(id.as_str()));
                        }
                        None => {
                            let empty = PROPVARIANT::default();
                            let _ = store.SetValue(&PKEY_AppUserModel_ID, &empty);
                            let _ = store.SetValue(&PKEY_AppUserModel_RelaunchIconResource, &empty);
                            let _ = store.SetValue(&PKEY_AppUserModel_RelaunchDisplayNameResource, &empty);
                            let _ = store.SetValue(&PKEY_AppUserModel_RelaunchCommand, &empty);
                        }
                    }
                    let _ = store.Commit();
                }
                if started {
                    CoUninitialize();
                }
            }
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

/// A Windows icon handle made from a picture (the caller owns it).
#[cfg(windows)]
pub fn icon_handle(image: &RgbaImage, size: u32) -> Option<isize> {
    imp::hicon(image, size).map(|h| h as isize)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gives another program's window (Character Map) the Pious icon, then
    /// saves a picture of the taskbar and title bar to PIOUS_ICON_SHOT:
    /// `PIOUS_ICON_SHOT=shot.png cargo test icon_on_a_real_window -- --ignored`
    #[test]
    #[ignore]
    #[cfg(windows)]
    fn icon_on_a_real_window() {
        let out = std::env::var("PIOUS_ICON_SHOT").unwrap();
        // Closed however the test ends (a failure would otherwise leave it open).
        struct Close(std::process::Child);
        impl Drop for Close {
            fn drop(&mut self) {
                let _ = self.0.kill();
            }
        }
        let child = Close(std::process::Command::new("charmap.exe").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().unwrap());
        let window = (0..40)
            .find_map(|_| {
                std::thread::sleep(std::time::Duration::from_millis(250));
                crate::core::process::window_of(child.0.id())
            })
            .expect("Character Map's window");
        let image = image::load_from_memory(PIOUS).unwrap().to_rgba8();
        let read_back = || unsafe {
            use windows::Win32::Foundation::HWND;
            use windows::Win32::Storage::EnhancedStorage::{PKEY_AppUserModel_ID, PKEY_AppUserModel_RelaunchIconResource};
            use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow};
            use windows_sys::Win32::UI::WindowsAndMessaging::{ICON_BIG, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_GETICON};
            let store: IPropertyStore = SHGetPropertyStoreForWindow(HWND(window as _)).unwrap();
            let id = store.GetValue(&PKEY_AppUserModel_ID).map(|v| v.to_string()).unwrap_or_default();
            let icon = store.GetValue(&PKEY_AppUserModel_RelaunchIconResource).map(|v| v.to_string()).unwrap_or_default();
            let mut big = 0usize;
            SendMessageTimeoutW(window as _, WM_GETICON, ICON_BIG as usize, 0, SMTO_ABORTIFHUNG, 300, &mut big);
            (id, icon, big)
        };
        set(window, "pious:test", &image);
        let (id, icon, big) = read_back();
        std::fs::write(&out, format!("app id: {id}\nicon file: {icon}\nwindow icon handle: {big:#x}\n")).unwrap();
        assert_eq!(id, "Pious.Roblox.pioustest", "the window has its own app ID");
        assert!(icon.ends_with(".ico,0") && std::path::Path::new(icon.trim_end_matches(",0")).is_file(), "{icon}");
        assert_ne!(big, 0, "the window has an icon");
        reset(window);
        let (id, _, big) = read_back();
        assert!(id.is_empty() && big == 0, "reset gives the window back its own ({id}, {big:#x})");
    }

    #[test]
    fn pious_icon_loads() {
        let image = image::load_from_memory(PIOUS).unwrap();
        assert!(image.width() >= 32);
        assert!(preview(&image.to_rgba8()).starts_with("data:image/png;base64,"));
    }
}
