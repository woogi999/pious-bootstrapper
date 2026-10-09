//! Pious's taskbar button: flashing for attention, a count badge (an
//! overlay icon) and progress. Windows may not show some of these (badges
//! are hidden with small taskbar buttons on Windows 10, and some taskbar
//! replacements show none), so every call quietly does nothing when it
//! can't.
//!
//! The badge and progress go through `ITaskbarList3`, a COM object, so they
//! must be called on the window's own (main) thread.

use image::{Rgba, RgbaImage};

/// Flashes the taskbar button until the window comes to the front (or
/// stops it, with `on` false).
pub fn flash(hwnd: isize, on: bool) {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{FLASHW_STOP, FLASHW_TIMERNOFG, FLASHW_TRAY, FLASHWINFO, FlashWindowEx};
        let info = FLASHWINFO {
            cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
            hwnd: hwnd as _,
            dwFlags: if on { FLASHW_TRAY | FLASHW_TIMERNOFG } else { FLASHW_STOP },
            uCount: if on { 3 } else { 0 },
            dwTimeout: 0,
        };
        FlashWindowEx(&info);
    }
    #[cfg(not(windows))]
    let _ = (hwnd, on);
}

/// A 3×5 pixel font for the badge's digits.
const DIGITS: [[u8; 5]; 11] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b010, 0b010, 0b010],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
    // "+"
    [0b000, 0b010, 0b111, 0b010, 0b000],
];

/// The badge picture: a red dot with the count (9+ above nine), 32×32.
pub fn badge_image(count: u32) -> RgbaImage {
    const SIZE: u32 = 32;
    let mut image = RgbaImage::new(SIZE, SIZE);
    let center = (SIZE as f32 - 1.0) / 2.0;
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let d = ((x as f32 - center).powi(2) + (y as f32 - center).powi(2)).sqrt();
        // Smooth edge.
        let alpha = (center + 0.5 - d).clamp(0.0, 1.0);
        if alpha > 0.0 {
            *pixel = Rgba([229, 72, 77, (alpha * 255.0) as u8]);
        }
    }
    let glyphs: Vec<usize> = if count > 9 { vec![9, 10] } else { vec![count as usize] };
    let scale = if glyphs.len() == 1 { 4 } else { 3 };
    let width = glyphs.len() as u32 * 3 * scale + (glyphs.len() as u32 - 1) * scale;
    let (left, top) = ((SIZE - width) / 2, (SIZE - 5 * scale) / 2);
    for (i, glyph) in glyphs.iter().enumerate() {
        let x0 = left + i as u32 * 4 * scale;
        for (row, bits) in DIGITS[*glyph].iter().enumerate() {
            for col in 0..3 {
                if bits & (0b100 >> col) != 0 {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            image.put_pixel(x0 + col * scale + dx, top + row as u32 * scale + dy, Rgba([255, 255, 255, 255]));
                        }
                    }
                }
            }
        }
    }
    image
}

#[cfg(windows)]
fn taskbar() -> Option<windows::Win32::UI::Shell::ITaskbarList3> {
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
    use windows::Win32::UI::Shell::{ITaskbarList3, TaskbarList};
    unsafe {
        let list: ITaskbarList3 = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER).ok()?;
        list.HrInit().ok()?;
        Some(list)
    }
}

/// Shows `count` on the taskbar button (0 takes the badge off). Call on the
/// window's thread.
pub fn set_badge(hwnd: isize, count: u32) {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, HICON};
        use windows::core::{PCWSTR, w};
        let Some(list) = taskbar() else { return };
        let window = HWND(hwnd as _);
        if count == 0 {
            let _ = list.SetOverlayIcon(window, HICON::default(), PCWSTR::null());
            return;
        }
        let Some(icon) = crate::core::playericon::icon_handle(&badge_image(count), 32) else { return };
        let icon = HICON(icon as _);
        let _ = list.SetOverlayIcon(window, icon, w!("New notifications"));
        // The taskbar keeps its own copy.
        let _ = DestroyIcon(icon);
    }
    #[cfg(not(windows))]
    let _ = (hwnd, count);
}

/// Progress on the taskbar button: `Some(0.0..=1.0)`, or `None` to clear.
/// Call on the window's thread.
pub fn set_progress(hwnd: isize, progress: Option<f64>) {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::Shell::{TBPF_NOPROGRESS, TBPF_NORMAL};
        let Some(list) = taskbar() else { return };
        let window = HWND(hwnd as _);
        match progress {
            Some(p) => {
                let _ = list.SetProgressState(window, TBPF_NORMAL);
                let _ = list.SetProgressValue(window, (p.clamp(0.0, 1.0) * 1000.0) as u64, 1000);
            }
            None => {
                let _ = list.SetProgressState(window, TBPF_NOPROGRESS);
            }
        }
    }
    #[cfg(not(windows))]
    let _ = (hwnd, progress);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_draws_digits() {
        let one = badge_image(1);
        let many = badge_image(42);
        assert_eq!(one.dimensions(), (32, 32));
        // White digit pixels in the middle, transparent corners.
        assert!(one.pixels().any(|p| p.0 == [255, 255, 255, 255]));
        assert_eq!(one.get_pixel(0, 0).0[3], 0);
        assert_ne!(one, many);
    }
}
