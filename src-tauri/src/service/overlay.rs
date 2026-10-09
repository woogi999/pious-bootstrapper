//! The in-game overlay: a hotkey brings Pious over whatever is on screen
//! (usually Roblox), the way Steam's overlay does.
//!
//! Opening: the window is placed over the monitor while still invisible,
//! the game behind it is blurred (live with Windows' own blur, or from a
//! still capture at any strength), then the window shows and the interface
//! animates in. Closing: the interface animates out first and tells Rust
//! when it's done, then the window hides and focus goes back to the game.

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder, WebviewWindow};

use super::{Service, Shared, Tone};
use crate::core::model::OverlayBlur;
use crate::core::{process, store};

pub const OVERLAY_WINDOW: &str = "overlay";

/// What the overlay was last opened with, while it's open: a window that
/// was only just made asks for it once its page is up (see
/// [`pending_show`]), since the event that opened it came too early.
static PENDING: std::sync::Mutex<Option<serde_json::Value>> = std::sync::Mutex::new(None);

pub fn pending_show() -> Option<serde_json::Value> {
    PENDING.lock().ok()?.clone()
}

impl Service {
    /// Makes the overlay window ahead of time so it opens instantly. Made
    /// from a background thread: building a window inside a main-thread
    /// call can hang on Windows, which left the overlay missing.
    pub fn prepare_overlay(&self) {
        if !self.read().bootstrapper.preferences.overlay.enabled {
            return;
        }
        let app = self.app().clone();
        tauri::async_runtime::spawn_blocking(move || {
            let _ = overlay_window(&app);
        });
    }

    pub fn toggle_overlay(self: &Shared) {
        let open = self.read().overlay_open;
        if open { self.hide_overlay() } else { self.show_overlay("full", None) }
    }

    /// Asks, over the game, how many seconds a manual clip should be.
    pub fn show_clip_prompt(self: &Shared, max: u32) {
        if self.read().overlay_open {
            let _ = self.app().emit_to(OVERLAY_WINDOW, "clip-prompt", max);
            return;
        }
        self.show_overlay("clip", Some(max));
    }

    /// Opens the overlay: `mode` "full" (everything) or "clip" (only the
    /// clip-length prompt, no blur).
    fn show_overlay(self: &Shared, mode: &str, clip_max: Option<u32>) {
        let app = self.app().clone();
        // Not made yet: make it off this thread (hotkeys arrive on the
        // window thread, where building a window can hang), then open it.
        let Some(window) = app.get_webview_window(OVERLAY_WINDOW) else {
            let me = self.clone();
            let mode = mode.to_owned();
            tauri::async_runtime::spawn_blocking(move || match overlay_window(me.app()) {
                Ok(_) => me.show_overlay(&mode, clip_max),
                Err(error) => me.toast(Tone::Negative, format!("Couldn't open the overlay ({error}).")),
            });
            return;
        };
        let (blur, strength, dim) = {
            let mut s = self.read();
            s.overlay_open = true;
            s.overlay_previous = process::foreground_window();
            let o = &s.bootstrapper.preferences.overlay;
            (o.blur, o.blur_strength, o.dim)
        };

        // Cover the monitor the cursor is on (where the game is).
        let monitor = app
            .cursor_position()
            .ok()
            .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
            .or_else(|| window.primary_monitor().ok().flatten());
        let (position, size) = match &monitor {
            Some(m) => (*m.position(), *m.size()),
            None => (PhysicalPosition::new(0, 0), PhysicalSize::new(1920, 1080)),
        };
        let _ = window.set_position(position);
        let _ = window.set_size(size);

        let blur = if mode == "clip" { OverlayBlur::Off } else { blur };
        let dim = if mode == "clip" { 0.0 } else { dim };
        // The still for the adjustable blur is taken before the overlay is
        // on screen, so it shows only the game.
        let backdrop = match blur {
            OverlayBlur::Adjustable => capture_backdrop(position.x, position.y, size.width as i32, size.height as i32),
            _ => None,
        };
        set_live_blur(&window, blur == OverlayBlur::Live);

        let payload = serde_json::json!({
            "backdrop": backdrop,
            "blur": if blur == OverlayBlur::Adjustable { strength } else { 0.0 },
            "dim": dim,
            "mode": mode,
            "clip_max": clip_max,
        });
        if let Ok(mut pending) = PENDING.lock() {
            *pending = Some(payload.clone());
        }
        let _ = window.show();
        let _ = window.set_always_on_top(true);
        let _ = window.set_focus();
        let _ = app.emit_to(OVERLAY_WINDOW, "overlay-show", payload);
    }

    /// Asks the overlay to animate out; it calls [`Self::finish_hide_overlay`].
    pub fn hide_overlay(&self) {
        if let Ok(mut pending) = PENDING.lock() {
            *pending = None;
        }
        let open = std::mem::replace(&mut self.read().overlay_open, false);
        if open {
            let _ = self.app().emit_to(OVERLAY_WINDOW, "overlay-hide", ());
        }
    }

    /// The overlay finished animating out: hide it and give focus back.
    pub fn finish_hide_overlay(&self) {
        if self.read().overlay_open {
            // Reopened while it was animating out.
            return;
        }
        if let Some(window) = self.app().get_webview_window(OVERLAY_WINDOW) {
            let _ = window.hide();
        }
        let previous = std::mem::take(&mut self.read().overlay_previous);
        if previous != 0 {
            // Off the window's thread, in case the game is slow to answer.
            std::thread::spawn(move || process::bring_to_front(previous));
        }
    }
}

fn overlay_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window(OVERLAY_WINDOW) {
        return Ok(window);
    }
    let window = WebviewWindowBuilder::new(app, OVERLAY_WINDOW, WebviewUrl::App("index.html".into()))
        .title("Pious Overlay")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .build()?;
    // The overlay never shows up in its own background capture.
    if let Ok(hwnd) = window.hwnd() {
        process::exclude_from_capture(hwnd.0 as isize, true);
    }
    Ok(window)
}

/// Windows' live blur behind the overlay (it blurs the game as it runs).
fn set_live_blur(window: &WebviewWindow, on: bool) {
    #[cfg(windows)]
    {
        let target = window.clone();
        let _ = window.run_on_main_thread(move || {
            let _ = window_vibrancy::clear_acrylic(&target);
            if on {
                let _ = window_vibrancy::apply_acrylic(&target, Some((8, 8, 10, 60)));
            }
        });
    }
    #[cfg(not(windows))]
    let _ = (window, on);
}

/// Captures the monitor and saves it for the overlay to blur. The file
/// name changes every time so the window never shows an old one.
fn capture_backdrop(x: i32, y: i32, width: i32, height: i32) -> Option<String> {
    let capture = process::capture(x, y, width, height, 3)?;
    let image = image::RgbaImage::from_raw(capture.width, capture.height, capture.rgba)?;
    let dir = store::data_dir().join("cache").join("overlay");
    let _ = std::fs::create_dir_all(&dir);
    // Only the newest still is ever needed.
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    let path = dir.join(format!("backdrop-{}.jpg", chrono::Utc::now().timestamp_millis()));
    image::DynamicImage::ImageRgba8(image).to_rgb8().save(&path).ok()?;
    Some(path.display().to_string())
}

