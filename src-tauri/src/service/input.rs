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
    /// Counts the game's frames while the FPS counter is on.
    pub fps: Option<crate::core::fps::Meter>,
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
            use crate::core::automation::ClickMode;
            inputhook::Config {
                watch: p.input_overlay.enabled,
                remaps,
                emoji: if p.emoji_shortcodes.in_roblox { roblox.clone() } else { HashSet::new() },
                buttons: s.automation.clicker.is_some() && matches!(p.autoclicker.mode, ClickMode::MouseHeld | ClickMode::OnClick),
                hotkeys: s.game_hotkeys.iter().enumerate().map(|(i, (keys, _))| (keys.clone(), i as u32)).collect(),
                // The games, and Pious itself while its overlay is open.
                hotkey_pids: roblox.iter().copied().chain(s.overlay_open.then(std::process::id)).collect(),
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
        let mut fps_checked = Instant::now();
        let mut sent_fps = u32::MAX;
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
            // clips).
            let (enabled, wanted, show_fps) = {
                let s = self.read();
                let o = &s.bootstrapper.preferences.input_overlay;
                let wanted = o.enabled
                    && (s.inputs.editing
                        || ((!o.only_in_game || s.foreground_roblox.is_some())
                            && (!o.only_while_capturing || s.capture.session.is_some())));
                (o.enabled, wanted, o.show_fps)
            };
            if wanted != shown {
                shown = wanted;
                self.show_input_overlay(wanted);
                sent_version = u64::MAX;
                sent_fps = u32::MAX;
            }

            // The FPS counter: watch the game in front (or any game).
            if fps_checked.elapsed() >= Duration::from_millis(250) {
                fps_checked = Instant::now();
                let watching = shown && show_fps;
                let area = if watching {
                    let pid = self.read().foreground_roblox;
                    tokio::task::spawn_blocking(move || {
                        let pid = pid.or_else(|| process::roblox_pids().first().copied())?;
                        process::window_of(pid).and_then(process::client_rect)
                    })
                    .await
                    .unwrap_or(None)
                } else {
                    None
                };
                let fps = {
                    let mut s = self.read();
                    if watching {
                        let meter = s.inputs.fps.get_or_insert_with(crate::core::fps::Meter::start);
                        meter.watch(area);
                        Some(if area.is_some() { meter.fps() } else { 0 })
                    } else {
                        s.inputs.fps = None;
                        None
                    }
                };
                if let Some(fps) = fps.filter(|f| *f != sent_fps) {
                    sent_fps = fps;
                    let _ = self.app().emit_to(INPUTS_WINDOW, "input-fps", fps);
                }
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
                let _ = self.app().run_on_main_thread(move || {
                    let Ok(window) = popup_window(&app) else { return };
                    // Under Roblox's chat box, at the top left of the game.
                    if let Some((x, y, _w, h)) = anchor {
                        let scale = window.scale_factor().unwrap_or(1.0);
                        let top = ((h as f64) * 0.36).min(300.0 * scale) as i32;
                        let _ = window.set_position(PhysicalPosition::new(x + (18.0 * scale) as i32, y + top));
                    }
                    let _ = window.show();
                    let _ = window.set_always_on_top(true);
                    let _ = app.emit_to(EMOJI_WINDOW, "emoji-list", payload);
                });
            }
            Event::Hotkey { id, pressed } => self.game_hotkey(id, pressed),
            Event::EmojiHide => {
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
        let _ = self.app().run_on_main_thread(move || {
            let Ok(window) = inputs_window(&app) else { return };
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
            let _ = window.set_size(LogicalSize::new(width.max(40.0), height.max(30.0)));
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
fn place(app: &AppHandle, window: &WebviewWindow, x: f32, y: f32) {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());
    if let Some(m) = monitor {
        let px = m.position().x + (m.size().width as f32 * x.clamp(0.0, 1.0)) as i32;
        let py = m.position().y + (m.size().height as f32 * y.clamp(0.0, 1.0)) as i32;
        let _ = window.set_position(PhysicalPosition::new(px, py));
    }
}

fn overlay_builder<'a>(app: &'a AppHandle, label: &'a str, title: &str) -> WebviewWindowBuilder<'a, tauri::Wry, AppHandle> {
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
