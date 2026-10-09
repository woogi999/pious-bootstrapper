//! Recording and clipping (see [`crate::core::recorder`]).
//!
//! The capture runs while a recording is going, or, with clips on, while
//! any Roblox window is open (that's the rolling buffer clips come from).
//! A small click-through notice over the game ("the HUD") shows when a
//! recording is running and when a video is saved.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use super::{Service, Shared, Tone};
use crate::core::model::{CaptureTarget, Recorder};
use crate::core::recorder::{self, Session};
use crate::core::{process, store};

pub const HUD_WINDOW: &str = "hud";

/// The recorder's state.
#[derive(Default)]
pub struct Capture {
    pub session: Option<Session>,
    /// The settings the session was started with.
    started_with: Option<Recorder>,
    encoder: Option<String>,
    /// Working encoders, once checked.
    pub encoders: Option<Vec<String>>,
    /// A recording: when it started, as (moment, stream time, game name).
    pub recording: Option<(Instant, f64, String)>,
    /// A manual clip waiting for its length: when the key was pressed.
    pending_clip: Option<Instant>,
    /// Videos being written; the session stays until they're done.
    cutting: u32,
    /// The capture was switched from GPU to GDI capture.
    gdi_fallback: bool,
    /// AMD's own capture failed on this PC: Desktop Duplication from now
    /// on (until Pious restarts).
    amd_failed: bool,
    pub installing: Option<(u64, Option<u64>)>,
    pub error: Option<String>,
    pub last_saved: Option<String>,
}

/// What the window shows about the recorder.
#[derive(Debug, Clone, Serialize)]
pub struct RecorderStatus {
    pub installed: bool,
    pub installing: Option<(u64, Option<u64>)>,
    pub capturing: bool,
    pub recording_since: Option<f64>,
    pub buffered: f64,
    pub encoder: Option<String>,
    pub encoders: Option<Vec<String>>,
    pub error: Option<String>,
    pub last_saved: Option<String>,
    pub folder: String,
}

fn ffmpeg() -> PathBuf {
    recorder::ffmpeg_path(&store::data_dir())
}

fn work_dir() -> PathBuf {
    store::data_dir().join("cache").join("capture")
}

/// Notices sent over the game, so a late re-send never replaces a newer one.
static HUD_SENT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Which FFmpeg and graphics cards (and drivers) the encoder list was
/// checked with (FFmpeg's path, size and date; each card's name and driver
/// version), so it's checked again when either changes.
fn ffmpeg_key(path: &std::path::Path) -> String {
    let meta = std::fs::metadata(path).ok();
    let modified = meta.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
    format!("{}|{}|{modified}|{}", path.display(), meta.map_or(0, |m| m.len()), recorder::graphics_cards())
}

/// Where videos go.
pub fn videos_folder(settings: &Recorder) -> PathBuf {
    settings
        .folder
        .clone()
        .unwrap_or_else(|| dirs::video_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("Videos")).join("Pious"))
}

/// The recorder's status, for the snapshot.
pub fn status_of(s: &super::State) -> RecorderStatus {
    let c = &s.capture;
    {
        RecorderStatus {
            installed: recorder::ffmpeg_ready(&store::data_dir()),
            installing: c.installing,
            capturing: c.session.is_some(),
            recording_since: c.recording.as_ref().map(|(at, ..)| at.elapsed().as_secs_f64()),
            buffered: c.session.as_ref().map(|x| x.status("").buffered_seconds).unwrap_or(0.0),
            encoder: c.encoder.clone(),
            encoders: c.encoders.clone(),
            error: c.error.clone(),
            last_saved: c.last_saved.clone(),
            folder: videos_folder(&s.bootstrapper.preferences.recorder).display().to_string(),
        }
    }
}

impl Service {

    /// Downloads FFmpeg, then checks which encoders work.
    pub async fn install_recorder(self: &Shared) {
        if self.read().capture.installing.is_some() {
            return;
        }
        self.mutate(|s| s.capture.installing = Some((0, None)));
        let me = self.clone();
        let result = recorder::install_ffmpeg(&store::data_dir(), move |done, total| {
            me.mutate_throttled(|s| s.capture.installing = Some((done, total)));
        })
        .await;
        self.mutate(|s| s.capture.installing = None);
        match result {
            Ok(()) => {
                self.toast(Tone::Positive, "Recording is ready.");
                self.check_encoders().await;
            }
            Err(error) => self.toast(Tone::Negative, error),
        }
    }

    pub async fn check_encoders(self: &Shared) -> Vec<String> {
        let path = ffmpeg();
        if !path.is_file() {
            return Vec::new();
        }
        let key = ffmpeg_key(&path);
        let found = tokio::task::spawn_blocking(move || recorder::probe_encoders(&path)).await.unwrap_or_default();
        crate::core::cache::save("encoders", &(key, found.clone()));
        self.mutate(|s| s.capture.encoders = Some(found.clone()));
        found
    }

    /// Starts or stops a recording.
    pub fn toggle_recording(self: &Shared) {
        let recording = self.read().capture.recording.is_some();
        if recording {
            self.stop_recording();
        } else {
            self.spawn(|s| async move { s.start_recording().await });
        }
    }

    async fn start_recording(self: &Shared) {
        if let Err(error) = self.ensure_capture().await {
            self.toast(Tone::Negative, &error);
            self.hud("error", &error);
            return;
        }
        let game = self.current_game_name();
        self.mutate(|s| {
            let now = Instant::now();
            let stream = s.capture.session.as_ref().map(|x| x.stream_time(now)).unwrap_or(0.0);
            s.capture.recording = Some((now, stream, game));
        });
        self.hud("recording", "Recording");
    }

    fn stop_recording(self: &Shared) {
        let stop = Instant::now();
        let Some((_, from, game)) = self.mutate(|s| {
            let r = s.capture.recording.take();
            if r.is_some() {
                s.capture.cutting += 1;
            }
            r
        }) else {
            return;
        };
        self.hud("saving", "Saving recording…");
        self.spawn(move |s| async move {
            let result = s.write_video(from, stop, &game, "Recording").await;
            s.mutate(|st| st.capture.cutting -= 1);
            s.saved(result, "Recording");
        });
    }

    /// Saves the last `seconds` (the setting when `None`) before `at`.
    pub fn save_clip(self: &Shared, seconds: Option<u32>, at: Instant) {
        let (seconds, ready) = {
            let s = self.read();
            let seconds = seconds.unwrap_or(s.bootstrapper.preferences.recorder.clip_seconds).clamp(1, 1800);
            (seconds, s.capture.session.is_some())
        };
        if !ready {
            let message = "Clips need the buffer: turn on clips in Settings → Recording, then play.";
            self.toast(Tone::Caution, message);
            self.hud("error", "Nothing to clip yet");
            return;
        }
        let game = self.current_game_name();
        let Some(to) = self.mutate(|s| {
            let to = s.capture.session.as_ref()?.stream_time(at);
            s.capture.cutting += 1;
            Some(to)
        }) else {
            return;
        };
        self.hud("saving", "Saving clip…");
        self.spawn(move |s| async move {
            let result = s.write_video(to - seconds as f64, at, &game, "Clip").await;
            s.mutate(|st| st.capture.cutting -= 1);
            s.saved(result, "Clip");
        });
    }

    /// The manual clip key: remember the moment, then ask how long.
    pub fn ask_clip_length(self: &Shared) {
        if self.read().capture.session.is_none() {
            self.toast(Tone::Caution, "Clips need the buffer: turn on clips in Settings → Recording, then play.");
            self.hud("error", "Nothing to clip yet");
            return;
        }
        let max = self.mutate(|s| {
            s.capture.pending_clip = Some(Instant::now());
            s.bootstrapper.preferences.recorder.buffer_seconds
        });
        self.show_clip_prompt(max);
    }

    /// The length for a pending manual clip (`None` cancels it).
    pub fn submit_clip(self: &Shared, seconds: Option<u32>) {
        let pending = self.mutate(|s| s.capture.pending_clip.take());
        if let (Some(at), Some(seconds)) = (pending, seconds) {
            self.save_clip(Some(seconds), at);
        }
    }

    /// Cuts `[from, until]` (stream time, moment) into a file.
    async fn write_video(self: &Shared, from: f64, until: Instant, game: &str, kind: &str) -> Result<PathBuf, String> {
        // Wait for the segment holding `until` to be finished.
        let to = self.read().capture.session.as_ref().map(|x| x.stream_time(until)).ok_or("The recorder stopped.")?;
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let covered = {
                let mut s = self.read();
                match s.capture.session.as_mut() {
                    Some(session) => {
                        session.poll();
                        session.covers(to)
                    }
                    None => return Err("The recorder stopped before the video was saved.".into()),
                }
            };
            if covered || Instant::now() > deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        let (job, settings) = {
            let s = self.read();
            let session = s.capture.session.as_ref().ok_or("The recorder stopped.")?;
            (session.plan_cut(from, to)?, s.bootstrapper.preferences.recorder.clone())
        };
        let out = videos_folder(&settings).join(recorder::file_name(&settings.file_name, game, kind, settings.container));
        let target = out.clone();
        tokio::task::spawn_blocking(move || job.run(&target)).await.map_err(|e| e.to_string())??;
        Ok(out)
    }

    fn saved(self: &Shared, result: Result<PathBuf, String>, kind: &str) {
        match result {
            Ok(path) => {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                self.mutate(|s| s.capture.last_saved = Some(path.display().to_string()));
                let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                crate::core::stats::record(&kind.to_lowercase(), serde_json::json!({ "game": self.current_game_name(), "bytes": bytes }));
                self.toast(Tone::Positive, format!("{kind} saved: {name}"));
                self.hud("saved", &format!("{kind} saved"));
            }
            Err(error) => {
                self.toast(Tone::Negative, &error);
                self.hud("error", &format!("{kind} wasn't saved"));
            }
        }
    }

    fn current_game_name(&self) -> String {
        let s = self.read();
        let pid = process::window_pid(process::foreground_window());
        let instance = s.instances.iter().find(|i| i.pid == Some(pid)).or_else(|| s.instances.first());
        instance.and_then(|i| s.bootstrapper.game(i.game)).map(|g| g.name.clone()).unwrap_or_else(|| "Roblox".into())
    }

    /// Starts the capture now if it isn't running.
    async fn ensure_capture(self: &Shared) -> Result<(), String> {
        if self.read().capture.session.is_some() {
            return Ok(());
        }
        self.start_capture(false).await
    }

    async fn start_capture(self: &Shared, gdi: bool) -> Result<(), String> {
        let path = tokio::task::spawn_blocking(ffmpeg).await.map_err(|e| e.to_string())?;
        if !path.is_file() {
            return Err(if recorder::built_in() {
                "Pious couldn't unpack its recorder into its data folder. Check there's free space, then try again.".into()
            } else {
                "Set up recording in Settings → Recording first (it downloads FFmpeg).".into()
            });
        }
        let known = self.read().capture.encoders.clone();
        let encoders = match known {
            Some(found) => found,
            // What worked last time with this same FFmpeg, so a recording
            // starts at once instead of re-checking every encoder first. An
            // encoder that fails for real is dropped (see `recorder_loop`).
            None => match crate::core::cache::load::<(String, Vec<String>)>("encoders").filter(|(key, list)| *key == ffmpeg_key(&path) && !list.is_empty()) {
                Some((_, list)) => {
                    self.mutate(|s| s.capture.encoders = Some(list.clone()));
                    list
                }
                None => self.check_encoders().await,
            },
        };
        let settings = self.read().bootstrapper.preferences.recorder.clone();
        let encoder = recorder::pick_encoder(&settings, &encoders).ok_or("No video encoder works on this PC.")?;

        // The game window (the one in front, else any).
        let pids = tokio::task::spawn_blocking(process::roblox_pids).await.unwrap_or_default();
        let front = process::window_pid(process::foreground_window());
        let pid = pids.iter().copied().find(|p| *p == front).or_else(|| pids.first().copied());
        let area = pid
            .and_then(process::window_of)
            .and_then(process::client_rect)
            .map(|(x, y, w, h)| (x, y, w as u32, h as u32));
        let area = match area {
            Some(area) => area,
            None => {
                // No game: the main screen.
                let monitor = self.app().primary_monitor().ok().flatten();
                match monitor {
                    Some(m) => (m.position().x, m.position().y, m.size().width, m.size().height),
                    None => (0, 0, 1920, 1080),
                }
            }
        };
        let mut source = recorder::locate(area, settings.target == CaptureTarget::GameMonitor, pid);
        if gdi {
            source.output = None;
        }
        source.amd_capture &= !self.read().capture.amd_failed;
        let work = work_dir();
        let session = tokio::task::spawn_blocking({
            let settings = settings.clone();
            let encoder = encoder.clone();
            move || Session::start(&path, &work, &settings, &encoder, &source)
        })
        .await
        .map_err(|e| e.to_string())??;
        self.mutate(|s| {
            s.capture.session = Some(session);
            s.capture.started_with = Some(settings);
            s.capture.encoder = Some(encoder);
            s.capture.gdi_fallback = gdi;
            s.capture.error = None;
        });
        Ok(())
    }

    /// Runs the buffer while it's wanted, keeps segments a recording or a
    /// pending clip needs, and restarts the capture when its settings
    /// change or it fails.
    pub(super) async fn recorder_loop(self: Shared) {
        // Leftovers from a previous run.
        let _ = std::fs::remove_dir_all(work_dir());
        // FFmpeg is inside Pious: unpack it now, off to the side, so the
        // first recording doesn't wait for it.
        let _ = tokio::task::spawn_blocking(|| recorder::unpack_built_in(&store::data_dir())).await;
        let mut started_at: Option<Instant> = None;
        let mut tick = 0u64;
        loop {
            tokio::time::sleep(Duration::from_millis(300)).await;
            tick += 1;
            let game_running = {
                let s = self.read();
                !s.instances.is_empty()
            } || (tick % 10 == 0 && !tokio::task::spawn_blocking(process::roblox_pids).await.unwrap_or_default().is_empty());

            enum Next {
                Nothing,
                Start,
                Stop,
                Restart(bool),
                Failed(String),
                /// AMD's capture failed or never produced a picture.
                AmdFailed,
            }
            let next = {
                let mut s = self.read();
                let settings = s.bootstrapper.preferences.recorder.clone();
                let wanted_by_clips = settings.clips && (game_running || s.capture.pending_clip.is_some());
                let wanted = wanted_by_clips || s.capture.recording.is_some() || s.capture.cutting > 0;
                let pending = s.capture.pending_clip;
                let recording_from = s.capture.recording.as_ref().map(|r| r.1);
                let changed = s.capture.started_with.as_ref().is_some_and(|w| *w != settings);
                let busy = s.capture.recording.is_some() || s.capture.cutting > 0 || pending.is_some();
                let gdi = s.capture.gdi_fallback;
                match s.capture.session.as_mut() {
                    None if wanted && settings.clips && game_running => Next::Start,
                    None => Next::Nothing,
                    Some(_) if !wanted => Next::Stop,
                    Some(session) => {
                        let amd = session.uses_amd_capture();
                        if let Err(error) = session.check() {
                            if amd { Next::AmdFailed } else { Next::Failed(error) }
                        } else {
                            session.poll();
                            let mut keep = recording_from;
                            if let Some(at) = pending {
                                let t = session.stream_time(at) - session.buffer_seconds as f64;
                                keep = Some(keep.map_or(t, |k: f64| k.min(t)));
                            }
                            session.keep_from = keep;
                            let stalled = session.uses_gpu_capture()
                                && !session.producing()
                                && started_at.is_some_and(|t| t.elapsed() > Duration::from_secs(6));
                            if stalled && amd {
                                Next::AmdFailed
                            } else if stalled {
                                Next::Restart(true)
                            } else if changed && !busy {
                                Next::Restart(gdi)
                            } else {
                                Next::Nothing
                            }
                        }
                    }
                }
            };
            match next {
                Next::Nothing => {}
                Next::Start => {
                    if ffmpeg().is_file() {
                        if let Err(error) = self.start_capture(false).await {
                            self.mutate(|s| s.capture.error = Some(error));
                        }
                        started_at = Some(Instant::now());
                    }
                }
                Next::Stop => {
                    let session = self.mutate(|s| {
                        s.capture.started_with = None;
                        s.capture.session.take()
                    });
                    if let Some(session) = session {
                        tokio::task::spawn_blocking(move || session.discard());
                    }
                }
                Next::Restart(gdi) => {
                    let session = self.mutate(|s| s.capture.session.take());
                    if let Some(session) = session {
                        tokio::task::spawn_blocking(move || session.discard());
                    }
                    if let Err(error) = self.start_capture(gdi).await {
                        self.mutate(|s| s.capture.error = Some(error));
                    }
                    started_at = Some(Instant::now());
                }
                Next::AmdFailed => {
                    // Same encoder, Desktop Duplication instead. A recording
                    // that was running goes on in the new capture, from its
                    // beginning (this happens within its first seconds).
                    let session = self.mutate(|s| {
                        s.capture.amd_failed = true;
                        if let Some(recording) = s.capture.recording.as_mut() {
                            recording.1 = 0.0;
                        }
                        s.capture.session.take()
                    });
                    if let Some(session) = session {
                        tokio::task::spawn_blocking(move || session.discard());
                    }
                    if let Err(error) = self.start_capture(false).await {
                        self.mutate(|s| s.capture.error = Some(error));
                    }
                    started_at = Some(Instant::now());
                }
                Next::Failed(error) => {
                    // The encoder itself failed (it passed the check but not
                    // with the game's frames): leave it out from now on, so
                    // the next start ("auto") uses the next one that works.
                    let encoder_failed = error.contains("encod") || error.contains("Output file is empty");
                    let (session, dropped) = self.mutate(|s| {
                        s.capture.error = Some(error.clone());
                        s.capture.recording = None;
                        let mut dropped = None;
                        if encoder_failed && s.bootstrapper.preferences.recorder.encoder == "auto" {
                            if let (Some(bad), Some(list)) = (s.capture.encoder.clone(), s.capture.encoders.as_mut()) {
                                if list.len() > 1 && !bad.starts_with("lib") {
                                    list.retain(|e| *e != bad);
                                    crate::core::cache::save("encoders", &(ffmpeg_key(&ffmpeg()), list.clone()));
                                    dropped = Some(bad);
                                }
                            }
                        }
                        (s.capture.session.take(), dropped)
                    });
                    if let Some(session) = session {
                        tokio::task::spawn_blocking(move || session.discard());
                    }
                    let error = match dropped {
                        Some(bad) => format!("{error} Pious stopped using {bad}; press record again."),
                        None => error,
                    };
                    self.toast(Tone::Negative, error);
                    // Don't retry in a tight loop.
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
            if tick % 3 == 0 && self.read().capture.recording.is_some() {
                let elapsed = self.read().capture.recording.as_ref().map(|r| r.0.elapsed().as_secs()).unwrap_or(0);
                let _ = self.app().emit_to(HUD_WINDOW, "hud-recording", elapsed);
            }
        }
    }

    // ── The notice over the game ─────────────────────────────────────────

    /// Shows a short notice over the game (kind: recording, saving, saved,
    /// error).
    pub fn hud(&self, kind: &str, text: &str) {
        // Macro notices always show: they say how to stop what's running.
        if !self.read().bootstrapper.preferences.recorder.notify && kind != "recording" && kind != "macro" {
            return;
        }
        let app = self.app().clone();
        let payload = json!({ "kind": kind, "text": text });
        let sent = HUD_SENT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        // Off the window thread: building a window inside a main-thread call
        // can hang on Windows (and window calls are safe from any thread).
        tauri::async_runtime::spawn_blocking(move || {
            let fresh = app.get_webview_window(HUD_WINDOW).is_none();
            let Ok(window) = hud_window(&app) else { return };
            // Top right of the screen the game is on.
            let monitor = app
                .cursor_position()
                .ok()
                .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
                .or_else(|| window.primary_monitor().ok().flatten());
            if let Some(m) = monitor {
                let scale = m.scale_factor();
                let width = (340.0 * scale) as u32;
                let height = (120.0 * scale) as u32;
                let _ = window.set_size(PhysicalSize::new(width, height));
                let _ = window.set_position(PhysicalPosition::new(
                    m.position().x + m.size().width as i32 - width as i32 - (16.0 * scale) as i32,
                    m.position().y + (16.0 * scale) as i32,
                ));
            }
            let _ = window.show();
            let _ = window.set_always_on_top(true);
            let _ = app.emit_to(HUD_WINDOW, "hud", payload.clone());
            // A window that was just made wasn't listening yet: say it again,
            // unless a newer notice has come since.
            if fresh {
                std::thread::sleep(std::time::Duration::from_millis(500));
                if HUD_SENT.load(std::sync::atomic::Ordering::Relaxed) == sent {
                    let _ = app.emit_to(HUD_WINDOW, "hud", payload);
                }
            }
        });
    }

    /// The HUD finished showing its notices.
    pub fn hide_hud(&self) {
        if self.read().capture.recording.is_some() {
            return;
        }
        if let Some(window) = self.app().get_webview_window(HUD_WINDOW) {
            let _ = window.hide();
        }
    }
}

fn hud_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window(HUD_WINDOW) {
        return Ok(window);
    }
    let window = WebviewWindowBuilder::new(app, HUD_WINDOW, WebviewUrl::App("index.html".into()))
        .title("Pious notices")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .focusable(false)
        .visible(false)
        .inner_size(340.0, 120.0)
        .build()?;
    let _ = window.set_ignore_cursor_events(true);
    // Never in recordings or screenshots.
    if let Ok(hwnd) = window.hwnd() {
        process::exclude_from_capture(hwnd.0 as isize, true);
    }
    Ok(window)
}
