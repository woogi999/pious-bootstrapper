//! The stats overlay: live numbers about the game drawn over it, like the
//! input overlay but for stats (Settings → Overlay → Game stats).
//!
//! What it can show, and where each number comes from:
//! - FPS: frames the game's window showed in the last second (`core::fps`).
//! - Ping, players and server FPS: Roblox's public server list, which
//!   reports each server's own average ping and frame rate. Roblox servers
//!   don't answer pings, so this is the server's figure, not a measurement
//!   from this PC, and private servers aren't listed (shown as "—").
//! - Location: where the server is (see `watch::locate_server`).
//! - Session: how long this game has been open.
//! - CPU and memory: what Roblox is using on this PC.
//! - Clock: the time.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{Emitter, Manager};

use super::input::{overlay_builder, place};
use super::{Service, Shared};
use crate::core::process;

pub const STATS_WINDOW: &str = "stats";

/// One reading, as the overlay draws it (`None`: not known, shown as "—").
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Reading {
    pub fps: Option<u32>,
    pub ping: Option<u32>,
    pub players: Option<(u32, u32)>,
    pub server_fps: Option<f32>,
    pub location: Option<String>,
    pub session_seconds: Option<u64>,
    pub cpu: Option<f32>,
    pub memory: Option<u64>,
    /// The server isn't in Roblox's public list (private or reserved).
    pub private: bool,
}

/// The server-list figures for one server, refreshed every half minute.
#[derive(Default)]
struct ServerFigures {
    key: Option<(u64, String)>,
    at: Option<Instant>,
    found: Option<crate::core::roblox::PublicServer>,
    looked: bool,
    busy: bool,
}

impl Service {
    pub(super) async fn stats_loop(self: Shared) {
        let mut shown = false;
        let mut meter: Option<crate::core::fps::Meter> = None;
        let mut usage: Option<crate::core::procstats::Usage> = None;
        let figures = Arc::new(Mutex::new(ServerFigures::default()));
        let mut last_sent: Option<Reading> = None;
        // The game's window and the processor share, looked up less often
        // than the frame rate is sent.
        let mut tick: u32 = 0;
        let mut area: Option<(i32, i32, i32, i32)> = None;
        let mut cpu: Option<f32> = None;
        loop {
            // Every 0.3 s while it shows, so FPS keeps up with the game.
            tokio::time::sleep(Duration::from_millis(if shown { 300 } else { 700 })).await;
            tick = tick.wrapping_add(1);
            let (wanted, items, editing) = {
                let s = self.read();
                let o = &s.bootstrapper.preferences.stats_overlay;
                let wanted = o.enabled && (s.stats_editing || !o.only_in_game || s.foreground_roblox.is_some());
                (wanted, o.items.clone(), s.stats_editing)
            };
            if wanted != shown {
                shown = wanted;
                self.show_stats_overlay(wanted);
                last_sent = None;
            }
            if !shown {
                meter = None;
                usage = None;
                continue;
            }
            let has = |item: &str| items.iter().any(|i| i == item);

            // The game: the one in front, else any.
            let front = self.read().foreground_roblox;
            let pid = match front {
                Some(pid) => Some(pid),
                None => tokio::task::spawn_blocking(process::roblox_pids).await.unwrap_or_default().first().copied(),
            };
            let mut reading = Reading::default();
            if let Some(pid) = pid {
                // Once a second is plenty for where the window is and what
                // Roblox uses; the frame rate goes out every tick.
                let slow_tick = tick % 3 == 1 || usage.as_ref().is_none_or(|u| u.pid() != pid);
                if has("fps") {
                    if slow_tick || area.is_none() {
                        area = tokio::task::spawn_blocking(move || process::window_of(pid).and_then(process::client_rect)).await.unwrap_or(None);
                    }
                    let m = meter.get_or_insert_with(crate::core::fps::Meter::start);
                    m.watch(area);
                    reading.fps = area.map(|_| m.fps());
                } else {
                    meter = None;
                }
                if has("cpu") || has("memory") {
                    if usage.as_ref().is_none_or(|u| u.pid() != pid) {
                        usage = Some(crate::core::procstats::Usage::new(pid));
                        cpu = None;
                    }
                    if slow_tick {
                        if let Some(u) = usage.as_mut() {
                            cpu = if has("cpu") { u.cpu().or(cpu) } else { None };
                        }
                    }
                    reading.cpu = cpu;
                    reading.memory = if has("memory") { crate::core::procstats::memory(pid) } else { None };
                }
                // What Pious knows about this window's game.
                let known = {
                    let s = self.read();
                    s.instances.iter().find(|i| i.pid == Some(pid)).map(|i| {
                        let place = s.bootstrapper.game(i.game).map(|g| g.place_id);
                        let private = matches!(i.server, crate::core::model::ServerChoice::Private(_));
                        (i.started, i.location.clone(), place, i.job.clone(), private)
                    })
                };
                if let Some((started, location, place, job, private)) = known {
                    reading.session_seconds = Some((chrono::Utc::now() - started).num_seconds().max(0) as u64);
                    reading.location = location;
                    if has("ping") || has("players") || has("server_fps") {
                        match (place, job) {
                            (Some(place), Some(job)) if !private => {
                                self.refresh_server_figures(&figures, place, job);
                                let f = figures.lock().unwrap_or_else(|e| e.into_inner());
                                match &f.found {
                                    Some(server) => {
                                        reading.ping = server.ping;
                                        reading.players = Some((server.playing, server.max_players));
                                        reading.server_fps = server.fps;
                                    }
                                    None => reading.private = f.looked,
                                }
                            }
                            _ => reading.private = private,
                        }
                    }
                }
            } else {
                meter = None;
                usage = None;
            }
            if last_sent.as_ref() != Some(&reading) || editing {
                let _ = self.app().emit_to(STATS_WINDOW, "stats", &reading);
                last_sent = Some(reading);
            }
        }
    }

    /// Looks the server up in Roblox's list again when it changed or every
    /// 30 seconds (in the background; the overlay keeps the last figures).
    fn refresh_server_figures(self: &Shared, figures: &Arc<Mutex<ServerFigures>>, place: u64, job: String) {
        {
            let mut f = figures.lock().unwrap_or_else(|e| e.into_inner());
            let key = Some((place, job.clone()));
            if f.key != key {
                *f = ServerFigures { key, ..Default::default() };
            }
            let fresh = f.at.is_some_and(|at| at.elapsed() < Duration::from_secs(30));
            if f.busy || fresh {
                return;
            }
            f.busy = true;
        }
        let me = self.clone();
        let figures = figures.clone();
        tauri::async_runtime::spawn(async move {
            let token = me.any_session(None).await;
            let found = crate::core::roblox::find_public_server(place, &job, 10, token.as_deref()).await;
            let mut f = figures.lock().unwrap_or_else(|e| e.into_inner());
            f.busy = false;
            if f.key.as_ref() != Some(&(place, job)) {
                return;
            }
            f.at = Some(Instant::now());
            if let Ok(found) = found {
                f.looked = true;
                f.found = found;
            }
        });
    }

    /// Shows or hides the stats overlay.
    fn show_stats_overlay(self: &Shared, show: bool) {
        let app = self.app().clone();
        if !show {
            let _ = app.emit_to(STATS_WINDOW, "stats-visible", false);
            let me = self.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_millis(320)).await;
                let still_hidden = {
                    let s = me.read();
                    let o = &s.bootstrapper.preferences.stats_overlay;
                    !(o.enabled && (s.stats_editing || !o.only_in_game || s.foreground_roblox.is_some()))
                };
                if still_hidden {
                    if let Some(window) = me.app().get_webview_window(STATS_WINDOW) {
                        let _ = window.hide();
                    }
                }
            });
            return;
        }
        let (x, y, recordable, editing) = {
            let s = self.read();
            let o = &s.bootstrapper.preferences.stats_overlay;
            (o.x, o.y, o.show_in_recordings, s.stats_editing)
        };
        // Made off the window thread (building a window inside a main-thread
        // call can hang on Windows).
        tauri::async_runtime::spawn_blocking(move || {
            let fresh = app.get_webview_window(STATS_WINDOW).is_none();
            let window = match app.get_webview_window(STATS_WINDOW) {
                Some(window) => window,
                None => match overlay_builder(&app, STATS_WINDOW, "Pious game stats").inner_size(320.0, 60.0).build() {
                    Ok(window) => window,
                    Err(_) => return,
                },
            };
            if fresh {
                std::thread::sleep(Duration::from_millis(400));
            }
            place(&app, &window, x, y);
            if let Ok(hwnd) = window.hwnd() {
                process::exclude_from_capture(hwnd.0 as isize, !recordable);
            }
            let _ = window.set_ignore_cursor_events(!editing);
            let _ = window.show();
            let _ = window.set_always_on_top(true);
            let _ = app.emit_to(STATS_WINDOW, "stats-visible", true);
            let _ = app.emit_to(STATS_WINDOW, "stats-edit", editing);
        });
    }

    /// Its settings changed: put it where it belongs now.
    pub fn stats_settings_changed(&self) {
        let (enabled, x, y, recordable, editing) = {
            let s = self.read();
            let o = &s.bootstrapper.preferences.stats_overlay;
            (o.enabled, o.x, o.y, o.show_in_recordings, s.stats_editing)
        };
        if !enabled {
            return;
        }
        let app = self.app().clone();
        let _ = self.app().run_on_main_thread(move || {
            let Some(window) = app.get_webview_window(STATS_WINDOW) else { return };
            place(&app, &window, x, y);
            if let Ok(hwnd) = window.hwnd() {
                process::exclude_from_capture(hwnd.0 as isize, !recordable);
            }
            let _ = window.set_ignore_cursor_events(!editing);
        });
    }

    /// The overlay measured itself: fit the window to it.
    pub fn stats_overlay_resize(&self, width: f64, height: f64) {
        if let Some(window) = self.app().get_webview_window(STATS_WINDOW) {
            super::input::resize_anchored(&window, width.max(40.0), height.max(20.0));
        }
    }

    /// Starts or ends moving it by dragging; ending saves the spot, as a
    /// fraction of its screen.
    pub fn stats_overlay_edit(&self, on: bool) {
        self.mutate(|s| s.stats_editing = on);
        let window = self.app().get_webview_window(STATS_WINDOW);
        if let Some(window) = &window {
            let _ = window.set_ignore_cursor_events(!on);
            let _ = self.app().emit_to(STATS_WINDOW, "stats-edit", on);
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
            s.bootstrapper.preferences.stats_overlay.x = x;
            s.bootstrapper.preferences.stats_overlay.y = y;
            s.dirty = true;
        });
    }
}
