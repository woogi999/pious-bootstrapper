//! Recording and clipping, built on FFmpeg.
//!
//! One capture serves both. The screen is grabbed with Windows' Desktop
//! Duplication (on the GPU, via FFmpeg's `ddagrab`), encoded by the
//! graphics card's own encoder when it has one, and written as a stream of
//! short MPEG-TS segments into a working folder. A clip is the last few
//! segments joined and trimmed, a recording is every segment between its
//! start and stop: both are cut without re-encoding, so saving costs
//! almost nothing and loses no quality. Old segments are deleted as the
//! buffer moves on.
//!
//! Sound is captured with WASAPI: only Roblox's own sound (process
//! loopback, Windows 10 2004+), or everything the PC plays, plus a
//! microphone. It's mixed (or kept as separate tracks) and fed to FFmpeg
//! through named pipes, padded with silence so it never drifts from the
//! picture.

use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;

use super::model::{AudioCodec, Container, GameAudio, RateControl, Recorder, VideoCodec};

/// Where FFmpeg comes from when Pious wasn't installed with it: the copy
/// published with Pious's releases (fast), then the original "essentials"
/// build (~115 MB zip, often slow).
pub const FFMPEG_URLS: [&str; 2] = [
    concat!("https://github.com/", "woogi999/pious-bootstrapper", "/releases/latest/download/ffmpeg.zip"),
    "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip",
];
/// How long each segment is. Clips are accurate to about a keyframe.
const SEGMENT_SECONDS: f64 = 2.0;
const SAMPLE_RATE: u32 = 48_000;

#[cfg(windows)]
const NO_WINDOW: u32 = 0x0800_0000;

fn hidden(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(NO_WINDOW);
    }
    command
}

// ── FFmpeg ───────────────────────────────────────────────────────────────

/// FFmpeg built into pious.exe (compressed by `build.rs`; empty when the
/// build had none), and its SHA-256.
static EMBEDDED: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ffmpeg.exe.zst"));
const EMBEDDED_SHA256: &str = include_str!(concat!(env!("OUT_DIR"), "/ffmpeg.sha256"));

/// Recording works without setting anything up: FFmpeg is inside Pious.
pub fn built_in() -> bool {
    !EMBEDDED.is_empty()
}

/// Where the built-in FFmpeg is unpacked: named by its hash, so a Pious
/// update with a newer FFmpeg unpacks next to (then replaces) the old one.
fn unpacked_path(data_dir: &Path) -> PathBuf {
    data_dir.join("tools").join(format!("ffmpeg-{}.exe", &EMBEDDED_SHA256[..EMBEDDED_SHA256.len().min(12)]))
}

static UNPACKED: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();

/// Unpacks the built-in FFmpeg if it isn't already (checked against its
/// hash, so a damaged or changed copy is replaced). Runs once per start;
/// callers at the same time wait for the first.
pub fn unpack_built_in(data_dir: &Path) -> Option<PathBuf> {
    if !built_in() {
        return None;
    }
    UNPACKED
        .get_or_init(|| {
            let target = unpacked_path(data_dir);
            let intact = std::fs::read(&target).ok().is_some_and(|bytes| sha256_hex(&bytes) == EMBEDDED_SHA256);
            if !intact {
                let unpacked = (|| -> std::io::Result<()> {
                    let mut decoder = zstd::stream::Decoder::new(EMBEDDED)?;
                    decoder.window_log_max(31)?;
                    let mut bytes = Vec::with_capacity(110 << 20);
                    std::io::copy(&mut decoder, &mut bytes)?;
                    if sha256_hex(&bytes) != EMBEDDED_SHA256 {
                        return Err(std::io::Error::other("the built-in FFmpeg is damaged"));
                    }
                    std::fs::create_dir_all(target.parent().unwrap_or(data_dir))?;
                    let partial = target.with_extension("part");
                    std::fs::write(&partial, &bytes)?;
                    std::fs::rename(&partial, &target)
                })();
                if unpacked.is_err() {
                    return None;
                }
            }
            // Older copies: an earlier built-in one, or a downloaded one.
            if let Ok(entries) = std::fs::read_dir(target.parent().unwrap_or(data_dir)) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let old = (name.starts_with("ffmpeg-") || name == "ffmpeg.exe") && name.ends_with(".exe") && entry.path() != target;
                    if old {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
            Some(target)
        })
        .clone()
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// FFmpeg: the one built into Pious, else a copy installed next to Pious
/// (older installs), else one downloaded later.
pub fn ffmpeg_path(data_dir: &Path) -> PathBuf {
    if let Some(path) = unpack_built_in(data_dir) {
        return path;
    }
    let bundled = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.join("ffmpeg.exe")));
    match bundled {
        Some(path) if path.is_file() => path,
        _ => data_dir.join("tools").join("ffmpeg.exe"),
    }
}

/// Recording is ready (or will be the moment it's needed), without
/// touching the disk for the built-in FFmpeg.
pub fn ffmpeg_ready(data_dir: &Path) -> bool {
    built_in() || {
        let bundled = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.join("ffmpeg.exe")));
        bundled.is_some_and(|p| p.is_file()) || data_dir.join("tools").join("ffmpeg.exe").is_file()
    }
}

/// Downloads FFmpeg and keeps only ffmpeg.exe. `progress` gets
/// (downloaded, total) bytes.
pub async fn install_ffmpeg(data_dir: &Path, progress: impl Fn(u64, Option<u64>)) -> Result<(), String> {
    use futures::StreamExt;
    let mut last = String::new();
    let mut found = None;
    for url in FFMPEG_URLS {
        match super::roblox::http()
            .get(url)
            .timeout(Duration::from_secs(1800))
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
        {
            Ok(response) => {
                found = Some(response);
                break;
            }
            Err(e) => last = format!("Couldn't download FFmpeg ({e})."),
        }
    }
    let response = found.ok_or(last)?;
    let total = response.content_length();
    let mut bytes = Vec::with_capacity(total.unwrap_or(0) as usize);
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("The FFmpeg download stopped ({e})."))?;
        bytes.extend_from_slice(&chunk);
        progress(bytes.len() as u64, total);
    }
    let target = data_dir.join("tools").join("ffmpeg.exe");
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("The FFmpeg download is damaged ({e})."))?;
        let index = (0..zip.len())
            .find(|&i| zip.by_index(i).map(|f| f.name() == "ffmpeg.exe" || f.name().ends_with("/bin/ffmpeg.exe")).unwrap_or(false))
            .ok_or("The FFmpeg download has no ffmpeg.exe.")?;
        let mut file = zip.by_index(index).map_err(|e| e.to_string())?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let partial = target.with_extension("part");
        let mut out = std::fs::File::create(&partial).map_err(|e| e.to_string())?;
        std::io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
        drop(out);
        std::fs::rename(&partial, &target).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Encoders this PC can use, best first, checked by encoding a few frames.
pub fn probe_encoders(ffmpeg: &Path) -> Vec<String> {
    const CANDIDATES: [&str; 12] = [
        "h264_nvenc",
        "hevc_nvenc",
        "av1_nvenc",
        "h264_amf",
        "hevc_amf",
        "av1_amf",
        "h264_qsv",
        "hevc_qsv",
        "av1_qsv",
        "libx264",
        "libx265",
        "libsvtav1",
    ];
    // All at once (each is its own short FFmpeg run): checking them one
    // after another made the first recording of a session take seconds to
    // start.
    let works = |encoder: &str| -> bool {
            hidden(Command::new(ffmpeg).args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=black:s=1280x720:r=30:d=0.2",
                "-pix_fmt",
                "yuv420p",
                "-c:v",
                encoder,
                "-f",
                "null",
                "-",
            ]))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };
    // One check per graphics vendor at a time (a card allows only so many
    // encoder sessions at once), the vendors side by side.
    let groups: Vec<Vec<&str>> = ["nvenc", "amf", "qsv", "lib"]
        .iter()
        .map(|kind| CANDIDATES.iter().copied().filter(|e| e.contains(kind)).collect())
        .collect();
    let found: Vec<String> = std::thread::scope(|scope| {
        let handles: Vec<_> = groups.iter().map(|group| scope.spawn(move || group.iter().filter(|e| works(e)).map(|e| (*e).to_owned()).collect::<Vec<_>>())).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    // Best first, as listed.
    CANDIDATES.iter().filter(|c| found.iter().any(|f| f == *c)).map(|e| (*e).to_owned()).collect()
}

/// The encoder to use: the chosen one, or the best working one for the
/// codec (graphics card first, then the processor).
pub fn pick_encoder(settings: &Recorder, available: &[String]) -> Option<String> {
    if settings.encoder != "auto" {
        return available.iter().find(|e| **e == settings.encoder).cloned();
    }
    let prefix = match settings.codec {
        VideoCodec::H264 => ["h264_", "libx264"],
        VideoCodec::Hevc => ["hevc_", "libx265"],
        VideoCodec::Av1 => ["av1_", "libsvtav1"],
    };
    available
        .iter()
        .find(|e| e.starts_with(prefix[0]))
        .or_else(|| available.iter().find(|e| e.starts_with(prefix[1])))
        .or_else(|| available.iter().find(|e| e.starts_with("h264_")))
        .or_else(|| available.iter().find(|e| *e == "libx264"))
        .cloned()
}

// ── Capture ──────────────────────────────────────────────────────────────

/// What to capture.
#[derive(Debug, Clone)]
pub struct Source {
    /// The Desktop Duplication output (monitor) index, when the area is on
    /// the main graphics card's screens.
    pub output: Option<u32>,
    /// The area, in screen pixels, and its offset within the output.
    pub area: (i32, i32, u32, u32),
    pub offset: (i32, i32),
    /// The game's process, for game-only sound.
    pub game_pid: Option<u32>,
    /// The size of the monitor the area is on (when known).
    pub monitor: Option<(u32, u32)>,
    /// AMD's own capture may be used (off after it failed once).
    pub amd_capture: bool,
}

impl Source {
    /// The area is the whole monitor (a fullscreen or maximized game, or
    /// "whole monitor" chosen).
    pub fn whole_monitor(&self) -> bool {
        let (_, _, w, h) = self.area;
        // Sizes are rounded down to even numbers for the encoder.
        self.offset == (0, 0) && self.monitor.is_some_and(|(mw, mh)| mw & !1 == w && mh & !1 == h)
    }
}

/// The graphics cards and their driver versions, as one line (what the
/// working encoders depend on).
#[cfg(windows)]
pub fn graphics_cards() -> String {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
    let mut out = Vec::new();
    unsafe {
        if let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() {
            for i in 0..8u32 {
                let Ok(adapter) = factory.EnumAdapters1(i) else { break };
                let Ok(desc) = adapter.GetDesc1() else { continue };
                let name = String::from_utf16_lossy(&desc.Description[..desc.Description.iter().position(|&c| c == 0).unwrap_or(0)]);
                // The driver's version changes with every driver update.
                let driver = adapter
                    .CheckInterfaceSupport(&<windows::Win32::Graphics::Dxgi::IDXGIDevice as windows::core::Interface>::IID)
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                out.push(format!("{name}:{:x}:{:x}:{driver}", desc.VendorId, desc.DeviceId));
            }
        }
    }
    out.join(";")
}

#[cfg(not(windows))]
pub fn graphics_cards() -> String {
    String::new()
}

/// Finds the monitor (Desktop Duplication output) showing a screen area.
#[cfg(windows)]
pub fn locate(area: (i32, i32, u32, u32), whole_monitor: bool, game_pid: Option<u32>) -> Source {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
    let (x, y, w, h) = area;
    let (cx, cy) = (x + w as i32 / 2, y + h as i32 / 2);
    let found = unsafe {
        CreateDXGIFactory1::<IDXGIFactory1>().ok().and_then(|factory| {
            let adapter = factory.EnumAdapters1(0).ok()?;
            (0..16u32).find_map(|i| {
                let output = adapter.EnumOutputs(i).ok()?;
                let desc = output.GetDesc().ok()?;
                let r = desc.DesktopCoordinates;
                (cx >= r.left && cx < r.right && cy >= r.top && cy < r.bottom).then_some((i, r))
            })
        })
    };
    match found {
        Some((index, r)) => {
            let monitor = (r.left, r.top, (r.right - r.left) as u32, (r.bottom - r.top) as u32);
            let (ax, ay, aw, ah) = if whole_monitor {
                monitor
            } else {
                // Clip to the monitor; even sizes for the encoder.
                let left = x.max(r.left);
                let top = y.max(r.top);
                let right = (x + w as i32).min(r.right);
                let bottom = (y + h as i32).min(r.bottom);
                (left, top, (right - left).max(2) as u32, (bottom - top).max(2) as u32)
            };
            Source {
                output: Some(index),
                area: (ax, ay, aw & !1, ah & !1),
                offset: (ax - r.left, ay - r.top),
                game_pid,
                monitor: Some((monitor.2, monitor.3)),
                amd_capture: true,
            }
        }
        None => Source { output: None, area: (x, y, w & !1, h & !1), offset: (0, 0), game_pid, monitor: None, amd_capture: false },
    }
}

#[cfg(not(windows))]
pub fn locate(area: (i32, i32, u32, u32), _whole_monitor: bool, game_pid: Option<u32>) -> Source {
    Source { output: None, area, offset: (0, 0), game_pid, monitor: None, amd_capture: false }
}

/// A finished segment: its file and where it sits in the stream (seconds).
#[derive(Debug, Clone)]
struct Segment {
    file: PathBuf,
    start: f64,
    end: f64,
}

/// A running capture.
pub struct Session {
    child: Child,
    gpu_capture: bool,
    /// Captured with AMD's own capture (see `Session::start`).
    amd_capture: bool,
    dir: PathBuf,
    started: Instant,
    ffmpeg: PathBuf,
    container: Container,
    audio_codec: AudioCodec,
    tracks: usize,
    hevc: bool,
    segments: Vec<Segment>,
    /// How far into the stream FFmpeg was when the session started
    /// (wall-clock seconds minus stream seconds; the smallest seen).
    lag: Option<f64>,
    csv_read: usize,
    audio: Option<AudioFeed>,
    pub buffer_seconds: u32,
    /// Segments from this stream time on are kept (a recording or a
    /// pending manual clip needs them).
    pub keep_from: Option<f64>,
    log: PathBuf,
}

/// Information about the running capture for the window.
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub encoder: String,
    pub buffered_seconds: f64,
}

impl Session {
    /// Whether any video has come out yet (Desktop Duplication waits for
    /// the screen to change before its first frame).
    pub fn producing(&self) -> bool {
        std::fs::read_dir(&self.dir)
            .map(|entries| entries.flatten().any(|e| e.file_name().to_string_lossy().ends_with(".ts") && e.metadata().map(|m| m.len() > 0).unwrap_or(false)))
            .unwrap_or(false)
    }

    pub fn uses_gpu_capture(&self) -> bool {
        self.gpu_capture
    }

    pub fn uses_amd_capture(&self) -> bool {
        self.amd_capture
    }

    pub fn start(ffmpeg: &Path, work: &Path, settings: &Recorder, encoder: &str, source: &Source) -> Result<Session, String> {
        let dir = work.join(format!("session-{}", chrono::Utc::now().timestamp_millis()));
        std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't make a folder for the recording ({e})."))?;

        // Sound first: FFmpeg opens the pipes as it starts.
        let (audio, pipes) = match AudioFeed::start(settings, source.game_pid) {
            Ok(feed) => {
                let pipes = feed.pipes.clone();
                (Some(feed), pipes)
            }
            Err(_) => (None, Vec::new()),
        };

        let mut args: Vec<String> = vec!["-hide_banner".into(), "-loglevel".into(), "warning".into(), "-y".into()];
        let fps = settings.fps.clamp(10, 240);
        let (_, _, w, h) = source.area;
        let hardware_frames;
        // AMD's encoder can't take Desktop Duplication's frames on the GPU,
        // so they used to be copied to the processor, converted and copied
        // back: on an AMD laptop chip that used more than a whole core and
        // still dropped to ~43 of 60 frames a second. AMD's own capture
        // hands its frames straight to the encoder (60 of 60, a quarter of
        // the processor time). It only captures whole monitors.
        let amd_direct = source.amd_capture
            && encoder.contains("amf")
            && !settings.ten_bit
            && source.output.is_some()
            && source.whole_monitor();
        match source.output {
            Some(index) if amd_direct => {
                hardware_frames = true;
                args.extend([
                    "-f".into(),
                    "lavfi".into(),
                    "-i".into(),
                    // "get_current" hands over the newest picture at once
                    // (the default mode waits and falls short of the frame
                    // rate); the fps filter below makes it steady.
                    format!("vsrc_amf=monitor_index={index}:framerate={fps}:capture_mode=get_current"),
                ]);
            }
            Some(index) => {
                hardware_frames = true;
                let (ox, oy) = source.offset;
                args.extend([
                    "-f".into(),
                    "lavfi".into(),
                    "-i".into(),
                    format!(
                        "ddagrab=output_idx={index}:framerate={fps}:draw_mouse={}:offset_x={ox}:offset_y={oy}:video_size={w}x{h}",
                        settings.cursor as u8
                    ),
                ]);
            }
            None => {
                // Not on the main graphics card's screens: GDI capture.
                hardware_frames = false;
                let (x, y, ..) = source.area;
                args.extend(
                    [
                        "-f",
                        "gdigrab",
                        "-framerate",
                        &fps.to_string(),
                        "-draw_mouse",
                        if settings.cursor { "1" } else { "0" },
                        "-offset_x",
                        &x.to_string(),
                        "-offset_y",
                        &y.to_string(),
                        "-video_size",
                        &format!("{w}x{h}"),
                        "-i",
                        "desktop",
                    ]
                    .map(str::to_owned),
                );
            }
        }
        for pipe in &pipes {
            args.extend(
                ["-f", "s16le", "-ar", &SAMPLE_RATE.to_string(), "-ac", "2", "-thread_queue_size", "1024", "-i", pipe]
                    .map(str::to_owned),
            );
        }

        // Picture: hardware frames go straight to a hardware encoder unless
        // they need resizing or 10-bit; everything else is downloaded.
        // AMD's encoder (AMF) refuses the desktop's frames handed over
        // directly ("Error submitting video frame", nothing encoded), so
        // only NVIDIA's takes them straight from the GPU.
        let gpu_direct = hardware_frames && settings.height.is_none() && !settings.ten_bit && encoder.contains("nvenc");
        let mut filters: Vec<String> = Vec::new();
        if amd_direct {
            filters.push(format!("fps={fps}"));
            if let Some(height) = settings.height.filter(|&t| t < h) {
                filters.push(format!("vpp_amf=w=-2:h={height}"));
            }
        } else if hardware_frames && !gpu_direct {
            filters.push("hwdownload".into());
            filters.push("format=bgra".into());
        }
        if !amd_direct {
            if let Some(height) = settings.height.filter(|&t| t < h) {
                filters.push(format!("scale=-2:{height}:flags=bicubic"));
            }
        }
        if !gpu_direct && !amd_direct {
            filters.push(format!("format={}", if settings.ten_bit { "p010le" } else { "nv12" }));
        }
        if encoder.contains("qsv") && !gpu_direct {
            filters.push("hwupload=extra_hw_frames=64".into());
        }
        args.extend(["-map".into(), "0:v".into()]);
        if !filters.is_empty() {
            args.extend(["-vf".into(), filters.join(",")]);
        }
        args.extend(["-c:v".into(), encoder.to_owned()]);
        args.extend(encoder_args(settings, encoder, fps));

        for (i, _) in pipes.iter().enumerate() {
            args.extend(["-map".into(), format!("{}:a", i + 1)]);
        }
        if !pipes.is_empty() {
            let codec = match settings.audio_codec {
                AudioCodec::Aac => "aac",
                AudioCodec::Opus => "libopus",
            };
            args.extend([
                "-c:a".into(),
                codec.into(),
                "-b:a".into(),
                format!("{}k", settings.audio_bitrate_kbps.clamp(64, 512)),
                "-af".into(),
                "aresample=async=1000".into(),
            ]);
            if settings.separate_tracks && pipes.len() == 2 {
                args.extend(["-metadata:s:a:0".into(), "title=Game".into(), "-metadata:s:a:1".into(), "title=Microphone".into()]);
            }
        }

        args.extend(
            [
                "-f",
                "segment",
                "-segment_time",
                &SEGMENT_SECONDS.to_string(),
                "-segment_format",
                "mpegts",
                "-segment_list",
                &dir.join("segments.csv").display().to_string(),
                "-segment_list_type",
                "csv",
                "-reset_timestamps",
                "0",
                &dir.join("seg%06d.ts").display().to_string(),
            ]
            .map(str::to_owned),
        );

        let log = dir.join("ffmpeg.log");
        let log_file = std::fs::File::create(&log).map_err(|e| e.to_string())?;
        let mut command = Command::new(ffmpeg);
        // Below normal priority: the game comes first. (The heavy work is
        // on the graphics card; this only keeps FFmpeg's own threads from
        // competing with Roblox's.)
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
            command.creation_flags(NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
        }
        let child = command.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(log_file)
            .spawn()
            .map_err(|e| format!("Couldn't start FFmpeg ({e})."))?;

        Ok(Session {
            child,
            gpu_capture: source.output.is_some(),
            amd_capture: amd_direct,
            dir,
            started: Instant::now(),
            ffmpeg: ffmpeg.to_owned(),
            container: settings.container,
            audio_codec: settings.audio_codec,
            tracks: pipes.len(),
            hevc: encoder.contains("hevc") || encoder.contains("x265"),
            segments: Vec::new(),
            lag: None,
            csv_read: 0,
            audio,
            buffer_seconds: settings.buffer_seconds.clamp(10, 1800),
            keep_from: None,
            log,
        })
    }

    /// Still capturing? On failure, the end of FFmpeg's log says why.
    pub fn check(&mut self) -> Result<(), String> {
        match self.child.try_wait() {
            Ok(None) => Ok(()),
            _ => {
                let log = std::fs::read_to_string(&self.log).unwrap_or_default();
                let reason = log.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("it stopped").to_owned();
                Err(format!("The recorder stopped: {reason}"))
            }
        }
    }

    /// Picks up newly finished segments and deletes ones the buffer no
    /// longer needs. Call every few hundred milliseconds.
    pub fn poll(&mut self) {
        use std::io::{Read, Seek, SeekFrom};
        let Ok(mut file) = std::fs::File::open(self.dir.join("segments.csv")) else { return };
        let now = self.started.elapsed().as_secs_f64();
        // Only what's new since last time (the list grows for as long as
        // the capture runs; reading it whole every few hundred ms added up).
        if file.seek(SeekFrom::Start(self.csv_read as u64)).is_err() {
            return;
        }
        let mut fresh = String::new();
        if file.read_to_string(&mut fresh).is_err() {
            return;
        }
        // A line still being written waits for the next poll.
        let complete = fresh.rfind('\n').map_or(0, |i| i + 1);
        for line in fresh[..complete].lines() {
            let parts: Vec<&str> = line.trim_end_matches('\r').split(',').collect();
            if parts.len() < 3 {
                continue;
            }
            let (Ok(start), Ok(end)) = (parts[1].parse::<f64>(), parts[2].parse::<f64>()) else { continue };
            let lag = now - end;
            self.lag = Some(self.lag.map_or(lag, |l: f64| l.min(lag)));
            self.segments.push(Segment { file: self.dir.join(parts[0]), start, end });
        }
        self.csv_read += complete;

        let stream_now = self.stream_time(Instant::now());
        let mut oldest_needed = stream_now - self.buffer_seconds as f64 - SEGMENT_SECONDS * 2.0;
        if let Some(keep) = self.keep_from {
            oldest_needed = oldest_needed.min(keep - SEGMENT_SECONDS);
        }
        self.segments.retain(|s| {
            let old = s.end < oldest_needed;
            if old {
                let _ = std::fs::remove_file(&s.file);
            }
            !old
        });
    }

    /// The stream time (seconds) of a moment.
    pub fn stream_time(&self, at: Instant) -> f64 {
        let since = at.saturating_duration_since(self.started).as_secs_f64();
        since - self.lag.unwrap_or(0.5)
    }

    pub fn status(&self, encoder: &str) -> Status {
        let first = self.segments.first().map(|s| s.start).unwrap_or(0.0);
        let last = self.segments.last().map(|s| s.end).unwrap_or(0.0);
        Status { encoder: encoder.to_owned(), buffered_seconds: (last - first).max(0.0) }
    }

    /// Whether the segments reach `time` yet.
    pub fn covers(&self, time: f64) -> bool {
        self.segments.last().is_some_and(|s| s.end >= time)
    }

    /// What it takes to join the segments between two stream times into
    /// one video. The job runs without the session (see [`CutJob::run`]).
    pub fn plan_cut(&self, from: f64, to: f64) -> Result<CutJob, String> {
        let parts: Vec<&Segment> = self.segments.iter().filter(|s| s.end > from && s.start < to).collect();
        let first = parts.first().ok_or("There's nothing recorded for that moment yet.")?;
        Ok(CutJob {
            ffmpeg: self.ffmpeg.clone(),
            dir: self.dir.clone(),
            files: parts.iter().map(|p| p.file.clone()).collect(),
            offset: (from - first.start).max(0.0),
            length: (to - from).max(0.5),
            container: self.container,
            aac: self.tracks > 0 && self.audio_codec == AudioCodec::Aac,
            hevc: self.hevc,
            opus: self.tracks > 0 && self.audio_codec == AudioCodec::Opus,
        })
    }

    /// Stops FFmpeg cleanly (it finishes the segment it's writing).
    pub fn finish(&mut self) {
        if let Some(stdin) = self.child.stdin.as_mut() {
            let _ = stdin.write_all(b"q");
            let _ = stdin.flush();
        }
        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.child.try_wait() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(audio) = self.audio.take() {
            audio.stop();
        }
        self.poll();
    }

    /// Stops and deletes the working files.
    pub fn discard(mut self) {
        self.finish();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        if let Some(audio) = self.audio.take() {
            audio.stop();
        }
    }
}

/// Joining segments into a finished video, without re-encoding.
pub struct CutJob {
    ffmpeg: PathBuf,
    dir: PathBuf,
    files: Vec<PathBuf>,
    offset: f64,
    length: f64,
    container: Container,
    aac: bool,
    hevc: bool,
    opus: bool,
}

impl CutJob {
    /// Writes the video to `out`. Blocks while FFmpeg works (a second or
    /// two; it only copies).
    pub fn run(&self, out: &Path) -> Result<(), String> {
        let list = self.dir.join(format!("cut-{}.txt", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)));
        let mut text = String::new();
        for file in &self.files {
            text.push_str(&format!("file '{}'\n", file.display().to_string().replace('\'', r"'\''")));
        }
        std::fs::write(&list, text).map_err(|e| e.to_string())?;
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Couldn't make the videos folder ({e})."))?;
        }
        let mut args: Vec<String> = [
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-ss",
            &format!("{:.3}", self.offset),
            "-f",
            "concat",
            "-safe",
            "0",
            "-i",
            &list.display().to_string(),
            "-t",
            &format!("{:.3}", self.length),
            "-map",
            "0",
            "-c",
            "copy",
            "-avoid_negative_ts",
            "make_zero",
        ]
        .map(str::to_owned)
        .to_vec();
        if self.container != Container::Mkv {
            if self.aac {
                args.extend(["-bsf:a".into(), "aac_adtstoasc".into()]);
            }
            // No "+faststart": it rewrites the whole file a second time to
            // move the index to the front, which only helps streaming over
            // the web and doubled the time long recordings took to save.
            // HEVC in MP4/MOV plays in more players tagged hvc1.
            if self.hevc {
                args.extend(["-tag:v".into(), "hvc1".into()]);
            }
            // Opus in MP4/MOV is still marked experimental in FFmpeg.
            if self.opus {
                args.extend(["-strict".into(), "experimental".into()]);
            }
        }
        args.push(out.display().to_string());
        let output = hidden(Command::new(&self.ffmpeg).args(&args)).stdin(Stdio::null()).output().map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&list);
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Couldn't save the video: {}", stderr.lines().last().unwrap_or("FFmpeg failed")))
        }
    }
}

/// Encoder options for the chosen quality, speed and rate control.
fn encoder_args(s: &Recorder, encoder: &str, fps: u32) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    let mut push = |items: &[&str]| a.extend(items.iter().map(|x| (*x).to_owned()));
    let q = s.quality.min(51).to_string();
    let b = format!("{}k", s.bitrate_kbps.max(500));
    let max = format!("{}k", s.max_bitrate_kbps.max(s.bitrate_kbps).max(500));
    let buf = format!("{}k", s.max_bitrate_kbps.max(s.bitrate_kbps).max(500) * 2);
    let gop = ((fps as f32 * s.keyframe_seconds.clamp(0.25, 10.0)).round() as u32).max(1).to_string();
    let speed = s.speed as usize; // 0 fastest … 4 best

    if encoder.contains("nvenc") {
        push(&["-preset", ["p1", "p3", "p4", "p5", "p7"][speed], "-tune", "hq"]);
        match s.rate_control {
            RateControl::Quality => push(&["-rc", "vbr", "-cq", &q, "-b:v", "0", "-maxrate", &max, "-bufsize", &buf]),
            RateControl::Vbr => push(&["-rc", "vbr", "-b:v", &b, "-maxrate", &max, "-bufsize", &buf]),
            RateControl::Cbr => push(&["-rc", "cbr", "-b:v", &b, "-bufsize", &buf]),
        }
    } else if encoder.contains("amf") {
        // Low latency: frames leave the encoder as they come instead of
        // queuing (what a live capture wants; measured at full frame rate).
        push(&["-quality", ["speed", "speed", "balanced", "quality", "quality"][speed], "-usage", "lowlatency"]);
        match s.rate_control {
            RateControl::Quality => push(&["-rc", "cqp", "-qp_i", &q, "-qp_p", &q, "-qp_b", &q]),
            RateControl::Vbr => push(&["-rc", "vbr_peak", "-b:v", &b, "-maxrate", &max]),
            RateControl::Cbr => push(&["-rc", "cbr", "-b:v", &b]),
        }
    } else if encoder.contains("qsv") {
        push(&["-preset", ["veryfast", "faster", "medium", "slow", "veryslow"][speed]]);
        match s.rate_control {
            RateControl::Quality => push(&["-global_quality", &q]),
            RateControl::Vbr => push(&["-b:v", &b, "-maxrate", &max]),
            RateControl::Cbr => push(&["-b:v", &b, "-maxrate", &b, "-minrate", &b]),
        }
    } else if encoder == "libsvtav1" {
        push(&["-preset", ["12", "10", "8", "6", "4"][speed]]);
        match s.rate_control {
            RateControl::Quality => push(&["-crf", &q]),
            _ => push(&["-b:v", &b]),
        }
    } else {
        // libx264 / libx265
        push(&["-preset", ["ultrafast", "superfast", "veryfast", "medium", "slow"][speed]]);
        match s.rate_control {
            RateControl::Quality => push(&["-crf", &q]),
            RateControl::Vbr => push(&["-b:v", &b, "-maxrate", &max, "-bufsize", &buf]),
            RateControl::Cbr => push(&["-b:v", &b, "-minrate", &b, "-maxrate", &b, "-bufsize", &buf]),
        }
    }
    if s.ten_bit && (encoder.starts_with("hevc") || encoder == "libx265") {
        push(&["-profile:v", "main10"]);
    }
    push(&["-g", &gop, "-force_key_frames", &format!("expr:gte(t,n_forced*{})", s.keyframe_seconds.clamp(0.25, 10.0))]);
    a
}

// ── Sound ────────────────────────────────────────────────────────────────

/// Samples from one capture: interleaved stereo f32 at 48 kHz.
type Samples = Arc<Mutex<VecDeque<f32>>>;

struct AudioFeed {
    stop: Arc<AtomicBool>,
    pipes: Vec<String>,
}

impl AudioFeed {
    #[cfg(windows)]
    fn start(settings: &Recorder, game_pid: Option<u32>) -> Result<AudioFeed, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let game = match settings.game_audio {
            GameAudio::Off => None,
            mode => Some(capture_thread(
                match (mode, game_pid) {
                    (GameAudio::GameOnly, Some(pid)) => Input::Process(pid),
                    _ => Input::System,
                },
                stop.clone(),
            )),
        };
        let mic = settings.mic.then(|| capture_thread(Input::Mic(settings.mic_device.clone()), stop.clone()));

        let gv = settings.game_volume.clamp(0.0, 4.0);
        let mv = settings.mic_volume.clamp(0.0, 4.0);
        let mut tracks: Vec<Vec<(Samples, f32)>> = Vec::new();
        match (game, mic) {
            (None, None) => return Err("No sound to record".into()),
            (Some(g), Some(m)) if settings.separate_tracks => {
                tracks.push(vec![(g, gv)]);
                tracks.push(vec![(m, mv)]);
            }
            (g, m) => tracks.push(g.map(|g| (g, gv)).into_iter().chain(m.map(|m| (m, mv))).collect()),
        }

        let tag = chrono::Utc::now().timestamp_millis();
        let mut pipes = Vec::new();
        for (i, sources) in tracks.into_iter().enumerate() {
            let name = format!(r"\\.\pipe\pious-audio-{tag}-{i}");
            let pipe = pipe::create(&name)?;
            let stop = stop.clone();
            std::thread::spawn(move || writer(pipe, sources, stop));
            pipes.push(name);
        }
        Ok(AudioFeed { stop, pipes })
    }

    #[cfg(not(windows))]
    fn start(_settings: &Recorder, _game_pid: Option<u32>) -> Result<AudioFeed, String> {
        Err("Sound capture is only available on Windows".into())
    }

    fn stop(self) {
        self.stop.store(true, Ordering::SeqCst);
        // Writers still waiting for FFmpeg to connect are let go.
        #[cfg(windows)]
        for name in &self.pipes {
            pipe::poke(name);
        }
    }
}

/// Records what one program plays (only it: Windows 10 2004 and later)
/// for `seconds`, as a WAV file: mono, 24 kHz, 16-bit.
#[cfg(windows)]
pub fn listen(pid: u32, seconds: f64) -> Result<Vec<u8>, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let queue = capture_thread(Input::Process(pid), stop.clone());
    let mut samples: Vec<f32> = Vec::new();
    let end = Instant::now() + Duration::from_secs_f64(seconds.clamp(0.5, 30.0));
    let start = Instant::now();
    while Instant::now() < end {
        std::thread::sleep(Duration::from_millis(50));
        // Windows sends nothing while it's quiet: fill those gaps with
        // silence so the length is right.
        let due = (start.elapsed().as_secs_f64() * SAMPLE_RATE as f64) as usize * 2;
        let mut q = queue.lock().unwrap_or_else(|e| e.into_inner());
        samples.extend(q.drain(..));
        drop(q);
        if samples.len() < due.saturating_sub(SAMPLE_RATE as usize) {
            samples.resize(due.saturating_sub(SAMPLE_RATE as usize / 2), 0.0);
        }
    }
    stop.store(true, Ordering::SeqCst);
    // Stereo 48 kHz → mono 24 kHz.
    let mono: Vec<i16> = samples
        .chunks_exact(4)
        .map(|c| ((c[0] + c[1] + c[2] + c[3]) / 4.0).clamp(-1.0, 1.0))
        .map(|s| (s * i16::MAX as f32) as i16)
        .collect();
    let rate = SAMPLE_RATE / 2;
    let data = (mono.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + data as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&(rate as u32).to_le_bytes());
    wav.extend_from_slice(&(rate as u32 * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data.to_le_bytes());
    for s in mono {
        wav.extend_from_slice(&s.to_le_bytes());
    }
    Ok(wav)
}

#[cfg(not(windows))]
pub fn listen(_pid: u32, _seconds: f64) -> Result<Vec<u8>, String> {
    Err("Listening needs Windows.".into())
}

#[cfg(windows)]
enum Input {
    Process(u32),
    System,
    Mic(Option<String>),
}

/// Captures one input on its own thread into a shared queue.
#[cfg(windows)]
fn capture_thread(input: Input, stop: Arc<AtomicBool>) -> Samples {
    let queue: Samples = Arc::new(Mutex::new(VecDeque::with_capacity(SAMPLE_RATE as usize)));
    let out = queue.clone();
    std::thread::spawn(move || {
        if let Err(error) = capture(input, &out, &stop) {
            eprintln!("audio capture stopped: {error}");
        }
    });
    queue
}

#[cfg(windows)]
fn capture(input: Input, out: &Samples, stop: &AtomicBool) -> Result<(), Box<dyn std::error::Error>> {
    use wasapi::{AudioClient, DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat, initialize_mta};
    initialize_mta().ok()?;
    let format = WaveFormat::new(32, 32, &SampleType::Float, SAMPLE_RATE as usize, 2, None);
    let mode = StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: 0 };
    let mut client = match &input {
        Input::Process(pid) => match AudioClient::new_application_loopback_client(*pid, true) {
            Ok(client) => client,
            // Older Windows: everything the PC plays instead.
            Err(_) => DeviceEnumerator::new()?.get_default_device(&Direction::Render)?.get_iaudioclient()?,
        },
        Input::System => DeviceEnumerator::new()?.get_default_device(&Direction::Render)?.get_iaudioclient()?,
        Input::Mic(id) => {
            let enumerator = DeviceEnumerator::new()?;
            let device = match id {
                Some(id) => {
                    let all = enumerator.get_device_collection(&Direction::Capture)?;
                    let mut found = None;
                    for device in &all {
                        let device = device?;
                        if device.get_id().ok().as_deref() == Some(id.as_str()) {
                            found = Some(device);
                            break;
                        }
                    }
                    match found {
                        Some(device) => device,
                        None => enumerator.get_default_device(&Direction::Capture)?,
                    }
                }
                None => enumerator.get_default_device(&Direction::Capture)?,
            };
            device.get_iaudioclient()?
        }
    };
    client.initialize_client(&format, &Direction::Capture, &mode)?;
    let event = client.set_get_eventhandle()?;
    let capture = client.get_audiocaptureclient()?;
    client.start_stream()?;
    let mut bytes: VecDeque<u8> = VecDeque::new();
    while !stop.load(Ordering::Relaxed) {
        if event.wait_for_event(200).is_err() {
            continue;
        }
        capture.read_from_device_to_deque(&mut bytes)?;
        let whole = bytes.len() / 4 * 4;
        let mut queue = out.lock().unwrap_or_else(|e| e.into_inner());
        for chunk in bytes.drain(..whole).collect::<Vec<u8>>().chunks_exact(4) {
            queue.push_back(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
        // Never more than half a second behind.
        let cap = SAMPLE_RATE as usize;
        if queue.len() > cap {
            let extra = queue.len() - cap / 2;
            queue.drain(..extra);
        }
    }
    let _ = client.stop_stream();
    Ok(())
}

/// Feeds one track to FFmpeg at real-time pace: the sources mixed, with
/// silence wherever a source has nothing (Windows sends nothing at all
/// while a program is quiet).
#[cfg(windows)]
fn writer(pipe: pipe::Pipe, sources: Vec<(Samples, f32)>, stop: Arc<AtomicBool>) {
    let Some(mut file) = pipe.connect() else { return };
    let start = Instant::now();
    let mut written: u64 = 0;
    let mut buf: Vec<u8> = Vec::with_capacity(8192);
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(10));
        let due = (start.elapsed().as_secs_f64() * SAMPLE_RATE as f64) as u64;
        let frames = due.saturating_sub(written).min(SAMPLE_RATE as u64) as usize;
        if frames == 0 {
            continue;
        }
        buf.clear();
        let mut locked: Vec<_> = sources.iter().map(|(q, v)| (q.lock().unwrap_or_else(|e| e.into_inner()), *v)).collect();
        for _ in 0..frames * 2 {
            let mut sum = 0.0f32;
            for (queue, volume) in locked.iter_mut() {
                sum += queue.pop_front().unwrap_or(0.0) * *volume;
            }
            let sample = (sum.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            buf.extend_from_slice(&sample.to_le_bytes());
        }
        drop(locked);
        if file.write_all(&buf).is_err() {
            return;
        }
        written += frames as u64;
    }
}

#[cfg(windows)]
mod pipe {
    use std::fs::File;
    use std::os::windows::io::FromRawHandle;

    use windows_sys::Win32::Foundation::{ERROR_PIPE_CONNECTED, GENERIC_WRITE, GetLastError, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, OPEN_EXISTING, PIPE_ACCESS_OUTBOUND};
    use windows_sys::Win32::System::Pipes::{ConnectNamedPipe, CreateNamedPipeW, PIPE_TYPE_BYTE, PIPE_WAIT};

    pub struct Pipe(isize);

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn create(name: &str) -> Result<Pipe, String> {
        let handle = unsafe {
            CreateNamedPipeW(wide(name).as_ptr(), PIPE_ACCESS_OUTBOUND, PIPE_TYPE_BYTE | PIPE_WAIT, 1, 1 << 20, 0, 0, std::ptr::null())
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err("Couldn't set up sound recording".into());
        }
        Ok(Pipe(handle as isize))
    }

    impl Pipe {
        /// Waits for FFmpeg to open the pipe.
        pub fn connect(self) -> Option<File> {
            let handle = self.0 as windows_sys::Win32::Foundation::HANDLE;
            let ok = unsafe { ConnectNamedPipe(handle, std::ptr::null_mut()) } != 0 || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
            let file = unsafe { File::from_raw_handle(handle as _) };
            ok.then_some(file)
        }
    }

    /// Connects to a pipe and lets go at once, releasing a writer that's
    /// still waiting for FFmpeg.
    pub fn poke(name: &str) {
        let handle =
            unsafe { CreateFileW(wide(name).as_ptr(), GENERIC_WRITE, 0, std::ptr::null(), OPEN_EXISTING, 0, std::ptr::null_mut()) };
        if handle != INVALID_HANDLE_VALUE {
            unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };
        }
    }
}

/// Microphones: (device ID, name).
#[cfg(windows)]
pub fn microphones() -> Vec<(String, String)> {
    use wasapi::{DeviceEnumerator, Direction, initialize_mta};
    let _ = initialize_mta();
    let Ok(enumerator) = DeviceEnumerator::new() else { return Vec::new() };
    let Ok(all) = enumerator.get_device_collection(&Direction::Capture) else { return Vec::new() };
    let mut out = Vec::new();
    for device in &all {
        if let Ok(device) = device {
            if let (Ok(id), Ok(name)) = (device.get_id(), device.get_friendlyname()) {
                out.push((id, name));
            }
        }
    }
    out
}

#[cfg(not(windows))]
pub fn microphones() -> Vec<(String, String)> {
    Vec::new()
}

/// The file name for a new video.
pub fn file_name(pattern: &str, game: &str, kind: &str, container: Container) -> String {
    let now = chrono::Local::now();
    let name = pattern
        .replace("{game}", game)
        .replace("{date}", &now.format("%Y-%m-%d").to_string())
        .replace("{time}", &now.format("%H-%M-%S").to_string())
        .replace("{kind}", kind);
    let clean: String = name.chars().map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { '_' } else { c }).collect();
    let clean = clean.trim().trim_end_matches('.');
    format!("{}.{}", if clean.is_empty() { "Pious" } else { clean }, container.extension())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_files_safely() {
        let name = file_name("{game} {kind}", "Jail/break: \"X\"", "Clip", Container::Mp4);
        assert!(name.starts_with("Jail_break_ _X_ Clip"));
        assert!(name.ends_with(".mp4"));
    }

    #[test]
    fn picks_hardware_first() {
        let available = vec!["libx264".to_owned(), "h264_nvenc".to_owned()];
        let settings = Recorder::default();
        assert_eq!(pick_encoder(&settings, &available).as_deref(), Some("h264_nvenc"));
        let hevc = Recorder { codec: VideoCodec::Hevc, ..Recorder::default() };
        // No HEVC encoder: falls back to H.264.
        assert_eq!(pick_encoder(&hevc, &available).as_deref(), Some("h264_nvenc"));
    }

    #[test]
    fn built_in_ffmpeg_unpacks_and_runs() {
        if !built_in() {
            eprintln!("this build has no FFmpeg inside (no installer\\vendor\\ffmpeg.exe)");
            return;
        }
        let dir = std::env::temp_dir().join(format!("pious-ffmpeg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = unpack_built_in(&dir).expect("unpacked");
        assert!(path.starts_with(&dir));
        let output = hidden(Command::new(&path).arg("-version")).output().unwrap();
        assert!(String::from_utf8_lossy(&output.stdout).starts_with("ffmpeg version"));
        assert!(ffmpeg_ready(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn whole_monitor_areas_only() {
        let mut source = Source { output: Some(0), area: (0, 0, 1920, 1080), offset: (0, 0), game_pid: None, monitor: Some((1920, 1080)), amd_capture: true };
        assert!(source.whole_monitor());
        source.monitor = Some((1921, 1081));
        assert!(source.whole_monitor(), "odd sizes are rounded down for the encoder");
        source.area = (100, 50, 1280, 720);
        source.offset = (100, 50);
        assert!(!source.whole_monitor());
    }

    /// A real capture of the main monitor with the best encoder this PC
    /// has: segments must appear and keep up with the frame rate. Needs a
    /// screen and FFmpeg (`installer\vendor\ffmpeg.exe` or PIOUS_FFMPEG):
    /// `cargo test --bin pious -- --ignored real_capture`.
    #[test]
    #[ignore]
    fn real_capture() {
        let ffmpeg = std::env::var_os("PIOUS_FFMPEG")
            .map(PathBuf::from)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("installer").join("vendor").join("ffmpeg.exe"));
        assert!(ffmpeg.is_file(), "no FFmpeg at {}", ffmpeg.display());
        let encoders = probe_encoders(&ffmpeg);
        let settings = Recorder { fps: 60, ..Recorder::default() };
        let encoder = pick_encoder(&settings, &encoders).expect("an encoder");
        let source = locate((0, 0, 1, 1), true, None);
        let work = std::env::temp_dir().join(format!("pious-capture-{}", std::process::id()));
        let mut session = Session::start(&ffmpeg, &work, &settings, &encoder, &source).unwrap();
        std::thread::sleep(Duration::from_secs(9));
        session.check().unwrap();
        session.poll();
        let covered: f64 = session.segments.iter().map(|s| s.end - s.start).sum();
        let log = std::fs::read_to_string(&session.log).unwrap_or_default();
        println!("encoder {encoder}, AMD capture {}, {covered:.1}s in {} segments\n{log}", session.uses_amd_capture(), session.segments.len());
        assert!(covered >= 4.0, "only {covered:.1}s captured");
        session.discard();
        let _ = std::fs::remove_dir_all(&work);
    }
}
