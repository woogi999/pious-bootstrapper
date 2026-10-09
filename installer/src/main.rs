//! Pious Setup: downloads Pious from GitHub Releases into a folder of its
//! own (loose files, FFmpeg next to the app), keeps it updated, and
//! uninstalls it. This is one program for every release: nothing about a
//! version is built into it.
//!
//!   pious-setup.exe                          install (or reinstall)
//!   pious-setup.exe --update --dir D --wait P  update the copy in D once
//!                                            process P (the old Pious) exits
//!   pious-setup.exe --uninstall              uninstall (also when this file
//!   uninstall.exe                            is named uninstall.exe)
//!
//! Installs are per user (no administrator rights): files go to
//! %LOCALAPPDATA%\Programs\Pious by default, with Start menu and desktop
//! shortcuts and an entry in Windows' installed apps.
//!
//! The release contract (see .github/workflows/release.yml): each release
//! has `manifest.json` listing `pious-X.Y.Z-win-x64.zip` (kind "app") and
//! `ffmpeg.zip` (kind "ffmpeg") with their SHA-256 and size.
//!
//! The install folder holds:
//!   pious.exe, ffmpeg.exe, docs\, themes\, plugins\, version.txt   (release files)
//!   pious-setup.exe, uninstall.exe                                  (copies of this program)
//!   install.json, install-files.txt                                 (what was installed)
//!   data\                                                           (the user's: never touched)

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};

const REPOSITORY: &str = "woogi999/pious-bootstrapper";
const APP_EXE: &str = "pious.exe";
const FFMPEG_EXE: &str = "ffmpeg.exe";
const SETUP_EXE: &str = "pious-setup.exe";
const UNINSTALLER: &str = "uninstall.exe";
const INSTALL_JSON: &str = "install.json";
const FILE_LIST: &str = "install-files.txt";
const VERSION_FILE: &str = "version.txt";
const DATA_DIR: &str = "data";
const LAYOUT: u32 = 2;
const UNINSTALL_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Pious";
const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Install,
    Update,
    Uninstall,
}

struct Options {
    mode: Mode,
    dir: Option<PathBuf>,
    wait: Option<u32>,
}

fn options() -> Options {
    let args: Vec<String> = std::env::args().collect();
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let exe_name = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    let mode = if args.iter().any(|a| a == "--update") {
        Mode::Update
    } else if args.iter().any(|a| a == "--uninstall") || exe_name == UNINSTALLER {
        Mode::Uninstall
    } else {
        Mode::Install
    };
    Options { mode, dir: value("--dir").map(PathBuf::from), wait: value("--wait").and_then(|p| p.parse().ok()) }
}

fn default_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("C:\\")).join("Programs").join("Pious")
}

/// Where Pious is installed now, according to Windows' app list.
fn installed_dir() -> Option<PathBuf> {
    registry::read(UNINSTALL_KEY, "InstallLocation").map(PathBuf::from).filter(|d| d.join(APP_EXE).is_file())
}

// ── Pure helpers ─────────────────────────────────────────────────────────

fn mb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_048_576.0)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// GitHub's own digest of an asset (`sha256:...`) wins over the manifest's.
fn choose_sha(digest: Option<&str>, manifest: &str) -> String {
    digest
        .and_then(|d| d.strip_prefix("sha256:"))
        .filter(|d| d.len() == 64)
        .unwrap_or(manifest)
        .to_ascii_lowercase()
}

/// `pious-1.2.3-win-x64.zip` -> `1.2.3`.
fn zip_version(name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    let version = lower.strip_prefix("pious-")?.strip_suffix("-win-x64.zip")?;
    (!version.is_empty()).then(|| version.to_owned())
}

fn version_key(version: &str) -> Vec<u64> {
    version.split(['.', '-', '+']).map(|p| p.parse().unwrap_or(0)).collect()
}

/// The release body as a list of short lines: bullets lose their marker,
/// headings and blank lines are dropped.
fn notes_lines(body: &str) -> Vec<String> {
    body.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.trim_start_matches(['-', '*', ' ']).replace("**", "").replace('`', ""))
        .filter(|l| !l.is_empty())
        .collect()
}

/// A path from an archive or from our own list, only if it stays inside the
/// install folder, isn't the user's data and isn't one of the installer's
/// own bookkeeping files.
fn safe_relative(path: &Path) -> Option<PathBuf> {
    use std::path::Component;
    let mut clean = PathBuf::new();
    for part in path.components() {
        match part {
            Component::Normal(p) => clean.push(p),
            Component::CurDir => {}
            _ => return None,
        }
    }
    let first = clean.components().next()?.as_os_str().to_string_lossy().to_lowercase();
    if first == DATA_DIR {
        return None;
    }
    let single = clean.components().count() == 1;
    if single && [INSTALL_JSON, FILE_LIST, SETUP_EXE, UNINSTALLER].contains(&first.as_str()) {
        return None;
    }
    Some(clean)
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut text = path.as_os_str().to_owned();
    text.push(suffix);
    PathBuf::from(text)
}

fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

fn parse_file_list(text: &str) -> Vec<PathBuf> {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).filter_map(|l| safe_relative(Path::new(l))).collect()
}

// ── GitHub ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

#[derive(Deserialize)]
struct Manifest {
    #[serde(default)]
    version: String,
    files: Vec<ManifestFile>,
}

#[derive(Deserialize)]
struct ManifestFile {
    name: String,
    kind: String,
    sha256: String,
    #[serde(default)]
    size: u64,
}

#[derive(Debug, Clone)]
struct Download {
    url: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Clone)]
struct Remote {
    version: String,
    notes: Vec<String>,
    app: Download,
    ffmpeg: Option<Download>,
}

fn download_for(file: &ManifestFile, assets: &[ApiAsset]) -> Result<Download, String> {
    let asset = assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(&file.name))
        .ok_or_else(|| format!("The release lists {} but doesn't have it.", file.name))?;
    Ok(Download {
        url: asset.browser_download_url.clone(),
        size: if asset.size > 0 { asset.size } else { file.size },
        sha256: choose_sha(asset.digest.as_deref(), &file.sha256),
    })
}

/// Joins a release and its manifest into what the installer needs.
fn resolve(release: ApiRelease, manifest: Manifest) -> Result<Remote, String> {
    let app = manifest
        .files
        .iter()
        .find(|f| f.kind == "app")
        .ok_or("The release has no app download.")?;
    let ffmpeg = manifest.files.iter().find(|f| f.kind == "ffmpeg");
    let version = if manifest.version.is_empty() {
        release.tag_name.trim_start_matches(['v', 'V']).to_owned()
    } else {
        manifest.version.clone()
    };
    Ok(Remote {
        version,
        notes: notes_lines(release.body.as_deref().unwrap_or("")),
        app: download_for(app, &release.assets)?,
        ffmpeg: match ffmpeg {
            Some(f) => Some(download_for(f, &release.assets)?),
            None => None,
        },
    })
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .user_agent(concat!("Pious-Setup/", env!("CARGO_PKG_VERSION")))
            .timeout_connect(Duration::from_secs(15))
            .timeout_read(Duration::from_secs(30))
            .timeout_write(Duration::from_secs(30))
            .build()
    })
}

fn describe(error: ureq::Error) -> String {
    match error {
        ureq::Error::Status(404, _) => "That file isn't on GitHub (no release has been published yet?).".into(),
        ureq::Error::Status(code, _) => format!("GitHub answered {code}."),
        ureq::Error::Transport(t) => format!("Couldn't reach GitHub ({t})."),
    }
}

/// Runs `step` up to three times, pausing a little between tries.
fn with_retries<T>(mut step: impl FnMut() -> Result<T, String>) -> Result<T, String> {
    let mut last = String::new();
    for attempt in 0..3u64 {
        match step() {
            Ok(value) => return Ok(value),
            Err(error) => last = error,
        }
        if attempt < 2 {
            std::thread::sleep(Duration::from_secs(attempt + 1));
        }
    }
    Err(last)
}

fn get_small(url: &str, accept: &str) -> Result<Vec<u8>, String> {
    with_retries(|| {
        let response = agent()
            .get(url)
            .set("Accept", accept)
            .set("X-GitHub-Api-Version", "2022-11-28")
            .call()
            .map_err(describe)?;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(8 * 1024 * 1024)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("The download was interrupted ({e})."))?;
        Ok(bytes)
    })
}

static REMOTE: Mutex<Option<Remote>> = Mutex::new(None);

fn fetch_remote() -> Result<Remote, String> {
    let url = format!("https://api.github.com/repos/{REPOSITORY}/releases/latest");
    let bytes = get_small(&url, "application/vnd.github+json")?;
    let release: ApiRelease =
        serde_json::from_slice(&bytes).map_err(|e| format!("GitHub's answer didn't make sense ({e})."))?;
    if release.draft || release.prerelease {
        return Err("The latest release isn't published yet.".into());
    }
    let manifest_asset = release
        .assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case("manifest.json"))
        .ok_or("The latest release has no manifest.json, so it can't be installed.")?;
    let manifest_bytes = get_small(&manifest_asset.browser_download_url, "application/octet-stream")?;
    let manifest: Manifest =
        serde_json::from_slice(&manifest_bytes).map_err(|e| format!("The release's manifest is damaged ({e})."))?;
    let remote = resolve(release, manifest)?;
    *REMOTE.lock().unwrap() = Some(remote.clone());
    Ok(remote)
}

fn remote() -> Result<Remote, String> {
    if let Some(r) = REMOTE.lock().unwrap().clone() {
        return Ok(r);
    }
    fetch_remote()
}

// ── Files next to the installer (offline fallback) ───────────────────────

#[derive(Debug, Clone)]
struct Local {
    app: PathBuf,
    ffmpeg: Option<PathBuf>,
    version: String,
}

fn local_source() -> Option<Local> {
    let folder = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let mut best: Option<(String, PathBuf)> = None;
    for entry in std::fs::read_dir(&folder).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(version) = zip_version(&name) else { continue };
        if best.as_ref().is_none_or(|(v, _)| version_key(&version) > version_key(v)) {
            best = Some((version, entry.path()));
        }
    }
    let (version, app) = best?;
    let ffmpeg = Some(folder.join("ffmpeg.zip")).filter(|p| p.is_file());
    Some(Local { app, ffmpeg, version })
}

// ── Commands ─────────────────────────────────────────────────────────────

#[tauri::command]
fn info() -> serde_json::Value {
    let opts = options();
    let existing = installed_dir();
    let dir = opts.dir.clone().or(existing.clone()).unwrap_or_else(default_dir);
    let local = local_source();
    json!({
        "mode": opts.mode,
        "dir": dir,
        "existing": existing,
        "existing_version": registry::read(UNINSTALL_KEY, "DisplayVersion"),
        "has_ffmpeg": dir.join(FFMPEG_EXE).is_file(),
        "elsewhere": processes::running_elsewhere(&dir),
        "local": local.map(|l| json!({
            "version": l.version,
            "file": l.app.file_name().map(|n| n.to_string_lossy().to_string()),
            "ffmpeg": l.ffmpeg.is_some(),
        })),
    })
}

/// The newest published release: version, notes and download sizes.
#[tauri::command]
async fn latest() -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let r = fetch_remote()?;
        Ok(json!({
            "version": r.version,
            "notes": r.notes,
            "app_size": r.app.size,
            "ffmpeg_size": r.ffmpeg.as_ref().map(|f| f.size).unwrap_or(0),
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn pick_dir(current: String) -> Option<PathBuf> {
    let start = PathBuf::from(&current);
    let parent = start.parent().map(Path::to_path_buf).unwrap_or(start);
    rfd::AsyncFileDialog::new()
        .set_title("Choose where to install Pious")
        .set_directory(parent)
        .pick_folder()
        .await
        .map(|f| {
            let picked = f.path().to_path_buf();
            // A folder of its own inside what was picked.
            if picked.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("pious")) {
                picked
            } else {
                picked.join("Pious")
            }
        })
}

fn progress(app: &AppHandle, stage: &str, done: u64, total: u64) {
    let _ = app.emit("progress", json!({ "stage": stage, "done": done, "total": total }));
}

#[tauri::command]
async fn install(app: AppHandle, dir: PathBuf, desktop: bool, offline: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || install_now(&app, &dir, desktop, None, offline))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn update(app: AppHandle) -> Result<(), String> {
    let opts = options();
    let dir = opts.dir.or_else(installed_dir).ok_or("Pious's folder wasn't given.")?;
    tauri::async_runtime::spawn_blocking(move || {
        install_now(&app, &dir, false, opts.wait, false)?;
        launch_app(&dir, true)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Streams `download` into `dest`, checking its SHA-256 as it goes.
fn fetch_file(app: &AppHandle, download: &Download, dest: &Path, label: &str) -> Result<(), String> {
    with_retries(|| {
        let response = agent().get(&download.url).call().map_err(describe)?;
        let total = response
            .header("Content-Length")
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(download.size);
        let mut reader = response.into_reader();
        let mut file = std::fs::File::create(dest).map_err(|e| format!("Couldn't write {label} ({e})."))?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 64 * 1024];
        let mut done = 0u64;
        let mut last = Instant::now() - Duration::from_secs(1);
        loop {
            let n = reader.read(&mut buffer).map_err(|e| format!("The download was interrupted ({e})."))?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            file.write_all(&buffer[..n]).map_err(|e| format!("Couldn't write {label} ({e})."))?;
            done += n as u64;
            if last.elapsed() >= Duration::from_millis(120) {
                last = Instant::now();
                progress(app, &format!("Downloading {label} ({} of {} MB)", mb(done), mb(total)), done, total);
            }
        }
        drop(file);
        progress(app, &format!("Verifying {label}"), 0, 0);
        if download.size > 0 && done != download.size {
            let _ = std::fs::remove_file(dest);
            return Err(format!("The {label} download was incomplete."));
        }
        if hex(&hasher.finalize()) != download.sha256 {
            let _ = std::fs::remove_file(dest);
            return Err(format!("The {label} download didn't match its published checksum."));
        }
        Ok(())
    })
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| format!("Couldn't read {} ({e}).", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(hex(&hasher.finalize()));
        }
        hasher.update(&buffer[..n]);
    }
}

/// Moves a finished `.part` file into place. A file still in use is moved
/// aside first (Windows allows renaming a running exe); the aside is returned.
fn place(partial: &Path, target: &Path) -> Result<Option<PathBuf>, String> {
    if std::fs::rename(partial, target).is_ok() {
        return Ok(None);
    }
    let aside = with_suffix(target, ".old");
    let _ = std::fs::remove_file(&aside);
    let _ = std::fs::rename(target, &aside);
    std::fs::rename(partial, target)
        .map_err(|e| format!("Couldn't replace {} ({e}). Close Pious and try again.", target.display()))?;
    Ok(Some(aside))
}

struct Extractor<'a> {
    app: &'a AppHandle,
    dir: &'a Path,
    total: u64,
    done: u64,
    files: Vec<PathBuf>,
    asides: Vec<PathBuf>,
    last: Instant,
}

impl Extractor<'_> {
    fn zip(&mut self, path: &Path) -> Result<(), String> {
        let file = std::fs::File::open(path).map_err(|e| format!("Couldn't open {} ({e}).", path.display()))?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("{} is damaged ({e}).", path.display()))?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            let Some(relative) = entry.enclosed_name().and_then(|n| safe_relative(&n)) else { continue };
            let target = self.dir.join(&relative);
            if entry.is_dir() {
                let _ = std::fs::create_dir_all(&target);
                continue;
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let name = relative.display().to_string();
            let partial = with_suffix(&target, ".part");
            let mut out = std::fs::File::create(&partial).map_err(|e| format!("Couldn't write {name} ({e})."))?;
            let mut buffer = vec![0u8; 1 << 20];
            loop {
                let n = entry.read(&mut buffer).map_err(|e| format!("The download is damaged ({e})."))?;
                if n == 0 {
                    break;
                }
                out.write_all(&buffer[..n]).map_err(|e| format!("Couldn't write {name} ({e})."))?;
                self.done += n as u64;
                if self.last.elapsed() >= Duration::from_millis(80) {
                    self.last = Instant::now();
                    progress(self.app, &format!("Extracting {name}"), self.done, self.total);
                }
            }
            drop(out);
            if let Some(aside) = place(&partial, &target)? {
                self.asides.push(aside);
            }
            self.files.push(relative);
        }
        Ok(())
    }
}

fn zip_size(path: &Path) -> u64 {
    std::fs::File::open(path)
        .ok()
        .and_then(|f| zip::ZipArchive::new(f).ok())
        .map(|mut z| (0..z.len()).filter_map(|i| z.by_index(i).ok().map(|f| f.size())).sum())
        .unwrap_or(0)
}

fn install_now(app: &AppHandle, dir: &Path, desktop: bool, wait: Option<u32>, offline: bool) -> Result<(), String> {
    let stage = std::env::temp_dir().join(format!("pious-setup-{}", std::process::id()));
    let result = install_inner(app, dir, desktop, wait, offline, &stage);
    let _ = std::fs::remove_dir_all(&stage);
    result
}

fn install_inner(
    app: &AppHandle,
    dir: &Path,
    desktop: bool,
    wait: Option<u32>,
    offline: bool,
    stage: &Path,
) -> Result<(), String> {
    std::fs::create_dir_all(stage).map_err(|e| format!("Couldn't use the temporary folder ({e})."))?;
    let previous: serde_json::Value = std::fs::read_to_string(dir.join(INSTALL_JSON))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}));
    let previous_ffmpeg = previous.get("ffmpeg_sha256").and_then(|v| v.as_str()).map(str::to_owned);
    let have_ffmpeg = dir.join(FFMPEG_EXE).is_file();

    // 1. Get the zips (downloaded and verified, or the ones next to us).
    let version;
    let app_zip: PathBuf;
    let mut ffmpeg_zip: Option<(PathBuf, String)> = None;
    // Releases since 1.0.1 have FFmpeg inside pious.exe and no ffmpeg.zip.
    let ships_ffmpeg;
    if offline {
        let local = local_source().ok_or("The files next to the installer are gone.")?;
        version = local.version.clone();
        app_zip = local.app.clone();
        ships_ffmpeg = local.ffmpeg.is_some();
        if let Some(z) = &local.ffmpeg {
            let sha = hash_file(z)?;
            if !have_ffmpeg || previous_ffmpeg.as_deref() != Some(sha.as_str()) {
                ffmpeg_zip = Some((z.clone(), sha));
            }
        }
    } else {
        let release = remote()?;
        version = release.version.clone();
        app_zip = stage.join("app.zip");
        fetch_file(app, &release.app, &app_zip, "Pious")?;
        ships_ffmpeg = release.ffmpeg.is_some();
        if let Some(f) = &release.ffmpeg {
            if !have_ffmpeg || previous_ffmpeg.as_deref() != Some(f.sha256.as_str()) {
                let path = stage.join("ffmpeg.zip");
                fetch_file(app, f, &path, "FFmpeg")?;
                ffmpeg_zip = Some((path, f.sha256.clone()));
            }
        }
    }

    // 2. The old Pious is closing for an update: give it a moment.
    if let Some(pid) = wait {
        progress(app, "Waiting for Pious to close", 0, 0);
        for _ in 0..150 {
            if !processes::alive(pid) {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    progress(app, "Closing Pious", 0, 0);
    processes::close_in(dir);

    // 3. Put the files in. Nothing under data\ is ever written or removed.
    std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't create {} ({e}).", dir.display()))?;
    let previous_files = std::fs::read_to_string(dir.join(FILE_LIST)).map(|t| parse_file_list(&t)).unwrap_or_default();
    let total = zip_size(&app_zip) + ffmpeg_zip.as_ref().map(|(p, _)| zip_size(p)).unwrap_or(0);
    let mut extractor = Extractor {
        app,
        dir,
        total,
        done: 0,
        files: Vec::new(),
        asides: Vec::new(),
        last: Instant::now(),
    };
    extractor.zip(&app_zip)?;
    if let Some((path, _)) = &ffmpeg_zip {
        extractor.zip(path)?;
    }
    let Extractor { mut files, mut asides, .. } = extractor;
    if ships_ffmpeg {
        if dir.join(FFMPEG_EXE).is_file() && !files.iter().any(|f| same_path(f, Path::new(FFMPEG_EXE))) {
            files.push(FFMPEG_EXE.into());
        }
    } else {
        // Built into pious.exe now: the separate copy goes.
        let _ = std::fs::remove_file(dir.join(FFMPEG_EXE));
    }

    progress(app, "Finishing", 0, 0);
    // Files the last release had and this one doesn't.
    for old in previous_files {
        if !files.iter().any(|f| same_path(f, &old)) {
            let _ = std::fs::remove_file(dir.join(&old));
            let _ = std::fs::remove_file(with_suffix(&dir.join(&old), ".old"));
        }
    }
    // The setup program is this program under other names.
    if let Ok(me) = std::env::current_exe() {
        for name in [SETUP_EXE, UNINSTALLER] {
            let target = dir.join(name);
            if same_path(&me, &target) {
                continue;
            }
            let partial = with_suffix(&target, ".part");
            if std::fs::copy(&me, &partial).is_ok() {
                if let Ok(Some(aside)) = place(&partial, &target) {
                    asides.push(aside);
                }
            }
        }
    }
    files.sort();
    let list: Vec<String> = files.iter().map(|f| f.display().to_string()).collect();
    std::fs::write(dir.join(FILE_LIST), list.join("\n")).map_err(|e| format!("Couldn't write the file list ({e})."))?;
    for aside in asides {
        let _ = std::fs::remove_file(aside);
    }
    let _ = std::fs::remove_file(dir.join("pious.old"));

    let exe = dir.join(APP_EXE);
    let start_menu = dirs::data_dir().map(|d| d.join("Microsoft\\Windows\\Start Menu\\Programs\\Pious.lnk"));
    if let Some(link) = &start_menu {
        shortcut(link, &exe, dir);
    }
    if desktop {
        if let Some(link) = dirs::desktop_dir().map(|d| d.join("Pious.lnk")) {
            shortcut(&link, &exe, dir);
        }
    }
    let size_kb = (total / 1024) as u32;
    let exe_text = exe.display().to_string();
    let dir_text = dir.display().to_string();
    let uninstall = format!("\"{}\" --uninstall", dir.join(SETUP_EXE).display());
    registry::write(UNINSTALL_KEY, "DisplayName", "Pious");
    registry::write(UNINSTALL_KEY, "DisplayVersion", &version);
    registry::write(UNINSTALL_KEY, "Publisher", "Pious");
    registry::write(UNINSTALL_KEY, "DisplayIcon", &exe_text);
    registry::write(UNINSTALL_KEY, "InstallLocation", &dir_text);
    registry::write(UNINSTALL_KEY, "UninstallString", &uninstall);
    registry::write(UNINSTALL_KEY, "QuietUninstallString", &uninstall);
    registry::write(UNINSTALL_KEY, "URLInfoAbout", "https://github.com/woogi999/pious-bootstrapper");
    registry::write_dword(UNINSTALL_KEY, "EstimatedSize", size_kb);
    registry::write_dword(UNINSTALL_KEY, "NoModify", 1);
    registry::write_dword(UNINSTALL_KEY, "NoRepair", 1);
    // Starting with Windows points at the copy just installed.
    if registry::read(RUN_KEY, "Pious").is_some() {
        registry::write(RUN_KEY, "Pious", &format!("\"{exe_text}\" --background"));
    }

    // Last: it marks the install as complete.
    let ffmpeg_sha = if ships_ffmpeg { ffmpeg_zip.map(|(_, sha)| sha).or(previous_ffmpeg) } else { None };
    let record = json!({ "version": version, "layout": LAYOUT, "ffmpeg_sha256": ffmpeg_sha });
    std::fs::write(dir.join(INSTALL_JSON), serde_json::to_string_pretty(&record).unwrap_or_default())
        .map_err(|e| format!("Couldn't write {INSTALL_JSON} ({e})."))?;
    progress(app, "Done", 1, 1);
    Ok(())
}

/// Makes a shortcut through Windows' own scripting (no extra files).
fn shortcut(link: &Path, target: &Path, dir: &Path) {
    let script = "$s = (New-Object -ComObject WScript.Shell).CreateShortcut($env:PIOUS_LINK); \
                  $s.TargetPath = $env:PIOUS_TARGET; $s.WorkingDirectory = $env:PIOUS_DIR; \
                  $s.IconLocation = $env:PIOUS_TARGET + ',0'; $s.Description = 'Pious'; $s.Save()";
    let mut command = std::process::Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
        .env("PIOUS_LINK", link)
        .env("PIOUS_TARGET", target)
        .env("PIOUS_DIR", dir);
    hide(&mut command);
    let _ = command.status();
}

fn hide(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000);
}

/// Whether this process runs as administrator (it normally does, see
/// `pious-setup.manifest`).
fn elevated() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut size = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

fn launch_app(dir: &Path, after_update: bool) -> Result<(), String> {
    if elevated() {
        // Never start Pious (and through it, Roblox) as administrator: Windows
        // Explorer starts it as the signed-in user. (It's already closed by
        // now, so "--after-update" has nothing left to wait for.)
        let _ = after_update;
        return std::process::Command::new("explorer.exe")
            .arg(dir.join(APP_EXE))
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Couldn't open Pious ({e})."));
    }
    let mut command = std::process::Command::new(dir.join(APP_EXE));
    command.current_dir(dir);
    if after_update {
        command.arg("--after-update");
    }
    command.spawn().map(|_| ()).map_err(|e| format!("Couldn't open Pious ({e})."))
}

#[tauri::command]
fn open_pious(dir: PathBuf) -> Result<(), String> {
    launch_app(&dir, false)
}

#[tauri::command]
async fn uninstall(app: AppHandle, remove_data: bool) -> Result<(), String> {
    let dir = options().dir.or_else(installed_dir).ok_or("Pious doesn't seem to be installed.")?;
    tauri::async_runtime::spawn_blocking(move || {
        progress(&app, "Closing Pious", 0, 4);
        processes::close_in(&dir);
        progress(&app, "Removing shortcuts", 1, 4);
        for link in [
            dirs::data_dir().map(|d| d.join("Microsoft\\Windows\\Start Menu\\Programs\\Pious.lnk")),
            dirs::desktop_dir().map(|d| d.join("Pious.lnk")),
        ]
        .into_iter()
        .flatten()
        {
            let _ = std::fs::remove_file(link);
        }
        registry::delete_tree(UNINSTALL_KEY);
        registry::delete_value(RUN_KEY, "Pious");
        progress(&app, "Removing files", 2, 4);
        // Only what Pious put there, in case the folder holds anything else.
        remove_installed_files(&dir);
        if remove_data {
            progress(&app, "Removing your data", 3, 4);
            let _ = std::fs::remove_dir_all(dir.join(DATA_DIR));
            if let Some(data) = dirs::data_local_dir().map(|d| d.join("Pious")) {
                let _ = std::fs::remove_dir_all(data);
            }
            let _ = std::fs::remove_dir(&dir);
        }
        progress(&app, "Done", 4, 4);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Removes the files the installer put in `dir` (and their leftovers from
/// updates), then the folders it made if nothing else is in them. User
/// data and anything the user added to plugins\ or themes\ stays.
fn remove_installed_files(dir: &Path) {
    let mut names: Vec<PathBuf> = vec![APP_EXE.into(), FFMPEG_EXE.into(), VERSION_FILE.into()];
    if let Ok(text) = std::fs::read_to_string(dir.join(FILE_LIST)) {
        names.extend(parse_file_list(&text));
    }
    let mut folders: Vec<PathBuf> = Vec::new();
    for name in &names {
        let target = dir.join(name);
        let _ = std::fs::remove_file(&target);
        for suffix in [".old", ".part"] {
            let _ = std::fs::remove_file(with_suffix(&target, suffix));
        }
        for parent in name.ancestors().skip(1) {
            if !parent.as_os_str().is_empty() {
                folders.push(parent.to_path_buf());
            }
        }
    }
    // Deepest first; remove_dir only succeeds on empty folders.
    folders.sort_by_key(|f| std::cmp::Reverse(f.components().count()));
    folders.dedup();
    for folder in folders {
        let _ = std::fs::remove_dir(dir.join(folder));
    }
    for name in [INSTALL_JSON, FILE_LIST, SETUP_EXE, UNINSTALLER, "pious.old", "pious.old.exe"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    // Fails (and leaves everything) if data\ or anything else is inside.
    let _ = std::fs::remove_dir(dir);
}

#[tauri::command]
fn quit(app: AppHandle) {
    // A copy of the uninstaller running from the temp folder removes itself.
    if options().mode == Mode::Uninstall {
        if let Ok(me) = std::env::current_exe() {
            if me.starts_with(std::env::temp_dir()) {
                let mut command = std::process::Command::new("cmd.exe");
                command.args(["/C", "ping 127.0.0.1 -n 3 > nul & del /f /q"]).arg(&me);
                hide(&mut command);
                let _ = command.spawn();
            }
        }
    }
    app.exit(0);
}

/// Asks Windows to run this program again as administrator, with the same
/// arguments. True when that copy started (this one should then quit).
///
/// Setup starts as whoever started it, so any program can start it (Pious
/// 1.0.0 starts it the plain way, which Windows refuses for a program that
/// demands administrator rights up front). It only asks when the signed-in
/// account is an administrator: elevating with another account's password
/// would install into that account's folders instead. If the user says no,
/// Setup carries on without: everything it does in the user's own folders
/// works either way.
fn relaunch_elevated() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION_TYPE, TOKEN_QUERY, TokenElevationType, TokenElevationTypeLimited};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let args: Vec<String> = std::env::args().skip(1).collect();
    if elevated() || args.iter().any(|a| a == "--elevated") {
        return false;
    }
    // A limited token of an administrator (UAC's split token) can be raised.
    let can_raise = unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            false
        } else {
            let mut kind: TOKEN_ELEVATION_TYPE = 0;
            let mut size = 0u32;
            let ok = GetTokenInformation(token, TokenElevationType, &mut kind as *mut _ as *mut _, std::mem::size_of::<TOKEN_ELEVATION_TYPE>() as u32, &mut size);
            CloseHandle(token);
            ok != 0 && kind == TokenElevationTypeLimited
        }
    };
    if !can_raise {
        return false;
    }
    let Ok(me) = std::env::current_exe() else { return false };
    let quote = |a: &str| if a.is_empty() || a.contains([' ', '\t', '"']) { format!("\"{}\"", a.replace('"', "\\\"")) } else { a.to_owned() };
    let params = args.iter().map(|a| quote(a)).chain(std::iter::once("--elevated".to_owned())).collect::<Vec<_>>().join(" ");
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let (verb, file, params) = (wide("runas"), wide(&me.display().to_string()), wide(&params));
    let result = unsafe { ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), file.as_ptr(), params.as_ptr(), std::ptr::null(), SW_SHOWNORMAL) };
    // Above 32: started. Anything else (the user said no): carry on here.
    result as isize > 32
}

fn main() {
    if relaunch_elevated() {
        return;
    }
    let opts = options();
    // The uninstaller can't delete the folder it runs from: run a copy
    // from the temp folder instead.
    if opts.mode == Mode::Uninstall {
        if let Ok(me) = std::env::current_exe() {
            if !me.starts_with(std::env::temp_dir()) {
                let dir = opts
                    .dir
                    .clone()
                    .or_else(|| me.parent().map(Path::to_path_buf))
                    .or_else(installed_dir)
                    .unwrap_or_else(default_dir);
                let copy = std::env::temp_dir().join(format!("pious-uninstall-{}.exe", std::process::id()));
                if std::fs::copy(&me, &copy).is_ok() {
                    let _ = std::process::Command::new(&copy).arg("--uninstall").arg("--dir").arg(&dir).spawn();
                    return;
                }
            }
        }
    }

    tauri::Builder::default()
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window_vibrancy::apply_acrylic(&window, Some((12, 12, 14, 150)));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![info, latest, pick_dir, install, update, open_pious, uninstall, quit])
        .run(tauri::generate_context!())
        .expect("error while running Pious Setup");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str, digest: Option<&str>) -> ApiAsset {
        ApiAsset {
            name: name.into(),
            browser_download_url: format!("https://example.invalid/{name}"),
            size: 10,
            digest: digest.map(str::to_owned),
        }
    }

    #[test]
    fn prefers_github_digest() {
        let digest = format!("sha256:{}", "AB".repeat(32));
        assert_eq!(choose_sha(Some(&digest), "feed"), "ab".repeat(32));
        assert_eq!(choose_sha(None, "FEED"), "feed");
        assert_eq!(choose_sha(Some("sha256:short"), "feed"), "feed");
    }

    #[test]
    fn reads_zip_names() {
        assert_eq!(zip_version("pious-1.2.3-win-x64.zip").as_deref(), Some("1.2.3"));
        assert_eq!(zip_version("Pious-1.2.3-Win-x64.ZIP").as_deref(), Some("1.2.3"));
        assert_eq!(zip_version("ffmpeg.zip"), None);
        assert!(version_key("1.10.0") > version_key("1.9.9"));
    }

    #[test]
    fn keeps_data_and_bookkeeping_out() {
        assert!(safe_relative(Path::new("docs/a.md")).is_some());
        assert!(safe_relative(Path::new("data/library.db")).is_none());
        assert!(safe_relative(Path::new("DATA")).is_none());
        assert!(safe_relative(Path::new("../x")).is_none());
        assert!(safe_relative(Path::new("install.json")).is_none());
        assert!(safe_relative(Path::new("plugins/uninstall.exe")).is_some());
        assert_eq!(parse_file_list("pious.exe\n\ndata/x\ndocs\\a.md\n").len(), 2);
    }

    #[test]
    fn turns_a_body_into_lines() {
        let lines = notes_lines("## 1.0\n\n- **Faster** `start`\n* Fixed it\nplain");
        assert_eq!(lines, vec!["Faster start", "Fixed it", "plain"]);
    }

    #[test]
    fn resolves_release_with_manifest() {
        let release = ApiRelease {
            tag_name: "v1.2.3".into(),
            body: Some("- hi".into()),
            draft: false,
            prerelease: false,
            assets: vec![
                asset("pious-1.2.3-win-x64.zip", Some(&format!("sha256:{}", "a".repeat(64)))),
                asset("ffmpeg.zip", None),
            ],
        };
        let manifest: Manifest = serde_json::from_str(
            r#"{"version":"1.2.3","files":[{"name":"pious-1.2.3-win-x64.zip","kind":"app","sha256":"bb","size":9},
               {"name":"ffmpeg.zip","kind":"ffmpeg","sha256":"CC","size":9}]}"#,
        )
        .unwrap();
        let remote = resolve(release, manifest).unwrap();
        assert_eq!(remote.version, "1.2.3");
        assert_eq!(remote.app.sha256, "a".repeat(64));
        assert_eq!(remote.ffmpeg.unwrap().sha256, "cc");
        assert_eq!(remote.notes, vec!["hi"]);
    }

    #[test]
    fn manifest_naming_a_missing_asset_fails() {
        let release = ApiRelease { tag_name: "v1".into(), body: None, draft: false, prerelease: false, assets: vec![] };
        let manifest: Manifest =
            serde_json::from_str(r#"{"files":[{"name":"x.zip","kind":"app","sha256":"aa"}]}"#).unwrap();
        assert!(resolve(release, manifest).is_err());
    }
}

mod registry {
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_SZ,
        RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegDeleteKeyValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn open(path: &str) -> Option<HKEY> {
        let mut key: HKEY = std::ptr::null_mut();
        let ok = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide(path).as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE | KEY_READ,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        (ok == 0).then_some(key)
    }

    pub fn write(path: &str, name: &str, value: &str) {
        let Some(key) = open(path) else { return };
        let data = wide(value);
        unsafe {
            RegSetValueExW(key, wide(name).as_ptr(), 0, REG_SZ, data.as_ptr() as *const u8, (data.len() * 2) as u32);
            RegCloseKey(key);
        }
    }

    pub fn write_dword(path: &str, name: &str, value: u32) {
        let Some(key) = open(path) else { return };
        unsafe {
            RegSetValueExW(key, wide(name).as_ptr(), 0, REG_DWORD, &value as *const u32 as *const u8, 4);
            RegCloseKey(key);
        }
    }

    pub fn read(path: &str, name: &str) -> Option<String> {
        let mut key: HKEY = std::ptr::null_mut();
        if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, wide(path).as_ptr(), 0, KEY_READ, &mut key) } != 0 {
            return None;
        }
        let mut buffer = vec![0u16; 1024];
        let mut size = (buffer.len() * 2) as u32;
        let ok = unsafe {
            RegGetValueW(
                key,
                std::ptr::null(),
                wide(name).as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buffer.as_mut_ptr() as *mut _,
                &mut size,
            )
        };
        unsafe { RegCloseKey(key) };
        if ok != 0 {
            return None;
        }
        let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        Some(String::from_utf16_lossy(&buffer[..len]))
    }

    pub fn delete_tree(path: &str) {
        unsafe {
            RegDeleteTreeW(HKEY_CURRENT_USER, wide(path).as_ptr());
            let (parent, leaf) = path.rsplit_once('\\').unwrap_or(("", path));
            let mut key: HKEY = std::ptr::null_mut();
            if RegOpenKeyExW(HKEY_CURRENT_USER, wide(parent).as_ptr(), 0, KEY_WRITE, &mut key) == 0 {
                windows_sys::Win32::System::Registry::RegDeleteKeyW(key, wide(leaf).as_ptr());
                RegCloseKey(key);
            }
        }
    }

    pub fn delete_value(path: &str, name: &str) {
        unsafe {
            RegDeleteKeyValueW(HKEY_CURRENT_USER, wide(path).as_ptr(), wide(name).as_ptr());
        }
    }
}

mod processes {
    use std::path::Path;

    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
        QueryFullProcessImageNameW, TerminateProcess,
    };

    /// Every running pious.exe: (process ID, full path).
    fn pious() -> Vec<(u32, std::path::PathBuf)> {
        let mut out = Vec::new();
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return out;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut more = Process32FirstW(snapshot, &mut entry) != 0;
            while more {
                let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                if name.eq_ignore_ascii_case(super::APP_EXE) {
                    let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID);
                    if !process.is_null() {
                        let mut buffer = vec![0u16; 1024];
                        let mut size = buffer.len() as u32;
                        if QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buffer.as_mut_ptr(), &mut size) != 0 {
                            out.push((entry.th32ProcessID, String::from_utf16_lossy(&buffer[..size as usize]).into()));
                        }
                        CloseHandle(process);
                    }
                }
                more = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
        }
        out
    }

    fn same_folder(exe: &Path, dir: &Path) -> bool {
        exe.parent()
            .map(|p| p.to_string_lossy().to_lowercase().trim_end_matches('\\').to_owned())
            == Some(dir.to_string_lossy().to_lowercase().trim_end_matches('\\').to_owned())
    }

    /// A copy of Pious running from somewhere else (it would keep the new
    /// one from opening).
    pub fn running_elsewhere(dir: &Path) -> bool {
        pious().iter().any(|(_, exe)| !same_folder(exe, dir))
    }

    /// Closes the Pious running from `dir`, if any.
    pub fn close_in(dir: &Path) {
        for (pid, exe) in pious() {
            if same_folder(&exe, dir) {
                unsafe {
                    let process = OpenProcess(PROCESS_TERMINATE, 0, pid);
                    if !process.is_null() {
                        TerminateProcess(process, 0);
                        CloseHandle(process);
                    }
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
    }

    pub fn alive(pid: u32) -> bool {
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return false;
            }
            let mut code = 0u32;
            let ok = GetExitCodeProcess(process, &mut code);
            CloseHandle(process);
            ok != 0 && code == 259
        }
    }
}
