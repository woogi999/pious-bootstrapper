//! What Pious does with the keyboard and mouse while you play: the input
//! overlay (keys drawn over the game), game keybinds (remaps) and emoji
//! shortcodes typed in Roblox. All three share one hook
//! ([`crate::core::inputhook`]); this keeps it set up and shows its windows.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use super::{Service, Shared};
use crate::core::inputhook::{self, Event};
use crate::core::process;

pub const INPUTS_WINDOW: &str = "inputs";
pub const EMOJI_WINDOW: &str = "emoji";

/// The input overlay's window state, kept between ticks.
#[derive(Default)]
pub struct InputWindows {
    /// The overlay is being moved (it takes the mouse while it is).
    pub editing: bool,
    /// Its size in logical pixels, as the window last measured it.
    pub size: Option<(f64, f64)>,
    /// It should be on screen (its window stays a moment longer while it
    /// animates out).
    pub visible: bool,
}

impl Service {
    /// The hook's configuration from the settings and the running games.
    pub fn sync_input_hook(&self, events: &mpsc::Sender<Event>, roblox: &HashSet<u32>) {
        let config = {
            let s = self.read();
            let p = &s.bootstrapper.preferences;
            let remaps = if p.keybinds.enabled {
                roblox
                    .iter()
                    .map(|pid| {
                        let instance = s.instances.iter().find(|i| i.pid == Some(*pid));
                        let account = instance.and_then(|i| i.account);
                        let place = instance.and_then(|i| s.bootstrapper.game(i.game)).map(|g| g.place_id);
                        (*pid, p.keybinds.resolve(account, place).into_iter().collect::<HashMap<_, _>>())
                    })
                    .filter(|(_, r)| !r.is_empty())
                    .collect()
            } else {
                HashMap::new()
            };
            inputhook::Config {
                watch: p.input_overlay.enabled,
                remaps,
                emoji: if p.emoji_shortcodes.in_roblox { roblox.clone() } else { HashSet::new() },
                hotkeys: s.game_hotkeys.iter().enumerate().map(|(i, (keys, _))| (keys.clone(), i as u32)).collect(),
                // The games, and Pious itself while its overlay is open.
                hotkey_pids: roblox.iter().copied().chain(s.overlay_open.then(std::process::id)).collect(),
                forward: !s.plugin_runtime.watching.is_empty(),
            }
        };
        inputhook::configure(config, events);
    }

    /// Keeps the hook, the input overlay and the emoji list going.
    pub(super) async fn input_loop(self: Shared) {
        let (events_tx, events_rx) = mpsc::channel::<Event>();
        // What the hook asks for, shown on the main thread.
        let me = self.clone();
        std::thread::Builder::new()
            .name("input-events".into())
            .spawn(move || {
                while let Ok(event) = events_rx.recv() {
                    me.on_input_event(event);
                }
            })
            .ok();

        let mut refreshed: Option<Instant> = None;
        let mut sent_version = u64::MAX;
        let mut shown = false;
        loop {
            tokio::time::sleep(Duration::from_millis(16)).await;
            let dirty = std::mem::take(&mut self.read().input_dirty);
            if dirty || refreshed.is_none_or(|t| t.elapsed() > Duration::from_secs(2)) {
                let roblox: HashSet<u32> =
                    tokio::task::spawn_blocking(process::roblox_pids).await.unwrap_or_default().into_iter().collect();
                refreshed = Some(Instant::now());
                self.sync_input_hook(&events_tx, &roblox);
            }

            // The overlay shows while it's on (and, if asked, only while a
            // game is in front, or only while Pious is recording or keeping
            // clips). The frame rate is the game stats overlay's.
            let (enabled, wanted) = {
                let s = self.read();
                let o = &s.bootstrapper.preferences.input_overlay;
                let wanted = o.enabled
                    && (s.inputs.editing
                        || ((!o.only_in_game || s.foreground_roblox.is_some())
                            && (!o.only_while_capturing || s.capture.session.is_some())));
                (o.enabled, wanted)
            };
            if wanted != shown {
                shown = wanted;
                self.show_input_overlay(wanted);
                sent_version = u64::MAX;
            }
            if !enabled {
                // Nothing to draw; check back less often.
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            }
            let version = inputhook::version();
            if shown && version != sent_version {
                sent_version = version;
                let _ = self.app().emit_to(INPUTS_WINDOW, "input-state", inputhook::held());
            }
        }
    }

    /// Hook settings changed: apply them now rather than on the next tick.
    pub fn input_settings_changed(&self) {
        self.apply_input_overlay_look();
    }

    fn on_input_event(self: &Shared, event: Event) {
        match event {
            Event::EmojiList { query, matches, selected, pid } => {
                let app = self.app().clone();
                let anchor = process::window_of(pid).and_then(process::client_rect);
                let payload = json!({ "query": query, "matches": matches, "selected": selected });
                // Made here (this thread), never inside a main-thread call:
                // building a window there can hang on Windows.
                let fresh = app.get_webview_window(EMOJI_WINDOW).is_none();
                let Ok(window) = popup_window(&app) else { return };
                // Under Roblox's chat box, at the top left of the game.
                if let Some((x, y, _w, h)) = anchor {
                    let scale = window.scale_factor().unwrap_or(1.0);
                    let top = ((h as f64) * 0.36).min(300.0 * scale) as i32;
                    let _ = window.set_position(PhysicalPosition::new(x + (18.0 * scale) as i32, y + top));
                }
                let _ = window.show();
                let _ = window.set_always_on_top(true);
                if let Ok(mut latest) = LATEST_EMOJI.lock() {
                    *latest = Some(payload.clone());
                }
                let _ = app.emit_to(EMOJI_WINDOW, "emoji-list", payload);
                // A window that was just made isn't listening yet: say the
                // newest list again once its page has loaded (never an older
                // one: typing goes on meanwhile).
                if fresh {
                    std::thread::spawn(move || {
                        for wait in [250, 600, 1200] {
                            std::thread::sleep(Duration::from_millis(wait));
                            if let Some(latest) = LATEST_EMOJI.lock().ok().and_then(|l| l.clone()) {
                                let _ = app.emit_to(EMOJI_WINDOW, "emoji-list", latest);
                            }
                        }
                    });
                }
            }
            Event::Hotkey { id, pressed } => self.game_hotkey(id, pressed),
            Event::Input { code, down } => self.plugin_input(code, down),
            Event::EmojiHide => {
                if let Ok(mut latest) = LATEST_EMOJI.lock() {
                    // An empty list: a late re-send hides it rather than
                    // bringing back an old one.
                    *latest = Some(json!({ "query": "", "matches": [], "selected": 0 }));
                }
                if let Some(window) = self.app().get_webview_window(EMOJI_WINDOW) {
                    let _ = window.hide();
                }
            }
        }
    }

    /// Shows or hides the input overlay. It animates out before its
    /// window goes.
    fn show_input_overlay(self: &Shared, show: bool) {
        let app = self.app().clone();
        let (x, y, recordable, editing) = self.mutate(|s| {
            s.inputs.visible = show;
            let o = &s.bootstrapper.preferences.input_overlay;
            (o.x, o.y, o.show_in_recordings, s.inputs.editing)
        });
        if !show {
            let _ = app.emit_to(INPUTS_WINDOW, "input-visible", false);
            let me = self.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_millis(320)).await;
                // Wanted back while it was leaving: leave it be.
                if me.read().inputs.visible {
                    return;
                }
                let app = me.app().clone();
                let _ = me.app().run_on_main_thread(move || {
                    if let Some(window) = app.get_webview_window(INPUTS_WINDOW) {
                        let _ = window.hide();
                    }
                });
            });
            return;
        }
        // Made off the window thread (building a window inside a main-thread
        // call can hang on Windows); window calls are safe from any thread.
        tauri::async_runtime::spawn_blocking(move || {
            let fresh = app.get_webview_window(INPUTS_WINDOW).is_none();
            let Ok(window) = inputs_window(&app) else { return };
            if fresh {
                // Let its page load, or it misses "input-visible".
                std::thread::sleep(Duration::from_millis(400));
            }
            place(&app, &window, x, y);
            if let Ok(hwnd) = window.hwnd() {
                process::exclude_from_capture(hwnd.0 as isize, !recordable);
            }
            let _ = window.set_ignore_cursor_events(!editing);
            let _ = window.show();
            let _ = window.set_always_on_top(true);
            let _ = app.emit_to(INPUTS_WINDOW, "input-visible", true);
        });
    }

    /// Re-applies where the overlay sits and whether recordings see it.
    fn apply_input_overlay_look(&self) {
        let (enabled, x, y, recordable, editing) = {
            let s = self.read();
            let o = &s.bootstrapper.preferences.input_overlay;
            (o.enabled, o.x, o.y, o.show_in_recordings, s.inputs.editing)
        };
        if !enabled {
            return;
        }
        let app = self.app().clone();
        let _ = self.app().run_on_main_thread(move || {
            let Some(window) = app.get_webview_window(INPUTS_WINDOW) else { return };
            place(&app, &window, x, y);
            if let Ok(hwnd) = window.hwnd() {
                process::exclude_from_capture(hwnd.0 as isize, !recordable);
            }
            let _ = window.set_ignore_cursor_events(!editing);
        });
    }

    /// The overlay measured itself: fit the window to it.
    pub fn input_overlay_resize(&self, width: f64, height: f64) {
        self.mutate(|s| s.inputs.size = Some((width, height)));
        if let Some(window) = self.app().get_webview_window(INPUTS_WINDOW) {
            resize_anchored(&window, width.max(40.0), height.max(30.0));
        }
    }

    /// Starts or ends moving the overlay by dragging. Ending saves where it
    /// was put, as a fraction of its screen.
    pub fn input_overlay_edit(&self, on: bool) {
        self.mutate(|s| s.inputs.editing = on);
        let window = self.app().get_webview_window(INPUTS_WINDOW);
        if let Some(window) = &window {
            let _ = window.set_ignore_cursor_events(!on);
            let _ = self.app().emit_to(INPUTS_WINDOW, "input-edit", on);
        }
        if on {
            return;
        }
        let Some(window) = window else { return };
        let (Ok(position), Ok(Some(monitor))) = (window.outer_position(), window.current_monitor()) else { return };
        let (mx, my) = (monitor.position().x as f32, monitor.position().y as f32);
        let (mw, mh) = (monitor.size().width.max(1) as f32, monitor.size().height.max(1) as f32);
        let x = ((position.x as f32 - mx) / mw).clamp(0.0, 0.98);
        let y = ((position.y as f32 - my) / mh).clamp(0.0, 0.98);
        self.mutate(|s| {
            s.bootstrapper.preferences.input_overlay.x = x;
            s.bootstrapper.preferences.input_overlay.y = y;
            s.dirty = true;
        });
    }
}

/// Puts the overlay at its spot on the screen the game is on (the one the
/// pointer is on, else the main screen).
pub(super) fn place(app: &AppHandle, window: &WebviewWindow, x: f32, y: f32) {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());
    if let Some(m) = monitor {
        let px = m.position().x + (m.size().width as f32 * x.clamp(0.0, 1.0)) as i32;
        let py = m.position().y + (m.size().height as f32 * y.clamp(0.0, 1.0)) as i32;
        // Fully on that screen, at the size it is now.
        let (w, h) = window.outer_size().map(|s| (s.width as i32, s.height as i32)).unwrap_or((0, 0));
        let (mx, my, mw, mh) = (m.position().x, m.position().y, m.size().width as i32, m.size().height as i32);
        let px = px.clamp(mx, (mx + mw - w).max(mx));
        let py = py.clamp(my, (my + mh - h).max(my));
        let _ = window.set_position(PhysicalPosition::new(px, py));
    }
}

/// Resizes an overlay window to `width` × `height` (logical pixels) so it
/// grows toward the middle of its screen: one on the right half keeps its
/// right edge where it is and grows left, one on the bottom half grows up.
/// It's always kept fully on its screen.
pub(super) fn resize_anchored(window: &WebviewWindow, width: f64, height: f64) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let (new_w, new_h) = ((width * scale).round() as i32, (height * scale).round() as i32);
    let (Ok(position), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        let _ = window.set_size(LogicalSize::new(width, height));
        return;
    };
    let Ok(Some(monitor)) = window.current_monitor() else {
        let _ = window.set_size(LogicalSize::new(width, height));
        return;
    };
    let (mx, my) = (monitor.position().x, monitor.position().y);
    let (mw, mh) = (monitor.size().width as i32, monitor.size().height as i32);
    let (old_w, old_h) = (size.width as i32, size.height as i32);
    let mut x = position.x;
    let mut y = position.y;
    if x + old_w / 2 > mx + mw / 2 {
        x = position.x + old_w - new_w;
    }
    if y + old_h / 2 > my + mh / 2 {
        y = position.y + old_h - new_h;
    }
    // On screen, whatever happens.
    x = x.clamp(mx, (mx + mw - new_w).max(mx));
    y = y.clamp(my, (my + mh - new_h).max(my));
    let _ = window.set_size(LogicalSize::new(width, height));
    if (x, y) != (position.x, position.y) {
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

/// The emoji list last sent, for re-sending to a window that was just made.
static LATEST_EMOJI: std::sync::Mutex<Option<serde_json::Value>> = std::sync::Mutex::new(None);

pub(super) fn overlay_builder<'a>(app: &'a AppHandle, label: &'a str, title: &str) -> WebviewWindowBuilder<'a, tauri::Wry, AppHandle> {
    WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title(title)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .focusable(false)
        .visible(false)
}

fn inputs_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window(INPUTS_WINDOW) {
        return Ok(window);
    }
    let window = overlay_builder(app, INPUTS_WINDOW, "Pious input overlay").inner_size(260.0, 200.0).build()?;
    let _ = window.set_ignore_cursor_events(true);
    Ok(window)
}

fn popup_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window(EMOJI_WINDOW) {
        return Ok(window);
    }
    let window = overlay_builder(app, EMOJI_WINDOW, "Pious emoji").inner_size(300.0, 330.0).build()?;
    let _ = window.set_ignore_cursor_events(true);
    // Never in recordings or screenshots.
    if let Ok(hwnd) = window.hwnd() {
        process::exclude_from_capture(hwnd.0 as isize, true);
    }
    Ok(window)
}
