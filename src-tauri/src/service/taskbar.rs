//! Pious's taskbar button, following the settings (Settings → Notifications
//! → Taskbar): a badge with how many pop-ups came while Pious wasn't in
//! front, a flash when something needs attention, and download progress.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use tauri::Manager;

use super::Service;
use crate::core::taskbar;

/// Pop-ups since Pious was last in front.
static UNSEEN: AtomicU32 = AtomicU32::new(0);
/// The progress last shown (×1000, +1; 0 = none), to only call Windows on
/// changes.
static SHOWN_PROGRESS: AtomicU64 = AtomicU64::new(0);

fn main_hwnd(service: &Service) -> Option<isize> {
    let window = service.app().get_webview_window("main")?;
    window.hwnd().ok().map(|h| h.0 as isize)
}

fn on_main_thread(service: &Service, f: impl FnOnce(isize) + Send + 'static) {
    let Some(hwnd) = main_hwnd(service) else { return };
    let _ = service.app().run_on_main_thread(move || f(hwnd));
}

pub fn set_badge(service: &Service, count: u32) {
    UNSEEN.store(count, Ordering::Relaxed);
    on_main_thread(service, move |hwnd| taskbar::set_badge(hwnd, count));
}

/// Something new arrived (`count` pop-ups): badge and flash, if wanted and
/// Pious isn't the window in front.
pub fn attention(service: &Service, count: u32) {
    let prefs = service.read().bootstrapper.preferences.taskbar.clone();
    let focused = service.app().get_webview_window("main").and_then(|w| w.is_focused().ok()).unwrap_or(false);
    if focused {
        return;
    }
    if prefs.badge {
        let total = UNSEEN.fetch_add(count, Ordering::Relaxed) + count;
        on_main_thread(service, move |hwnd| taskbar::set_badge(hwnd, total));
    }
    if prefs.flash {
        if let Some(hwnd) = main_hwnd(service) {
            taskbar::flash(hwnd, true);
        }
    }
}

/// Pious came to the front: everything's been seen.
pub fn seen(service: &Service) {
    if UNSEEN.swap(0, Ordering::Relaxed) > 0 {
        on_main_thread(service, |hwnd| taskbar::set_badge(hwnd, 0));
    }
}

/// Shows download progress (`None` clears it), only when it changed.
pub fn progress(service: &Service, value: Option<f64>) {
    let wanted = service.read().bootstrapper.preferences.taskbar.progress;
    let value = value.filter(|_| wanted);
    let key = value.map_or(0, |v| (v.clamp(0.0, 1.0) * 1000.0) as u64 + 1);
    if SHOWN_PROGRESS.swap(key, Ordering::Relaxed) != key {
        on_main_thread(service, move |hwnd| taskbar::set_progress(hwnd, value));
    }
}
