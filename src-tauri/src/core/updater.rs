//! Updates from GitHub Releases.
//!
//! Each release carries `manifest.json` and the zips it lists (the loose-file
//! install), `Pious-Setup.exe` (the reusable installer) and `pious.exe` (the
//! app alone, for copies run without installing). The app asks GitHub for
//! the latest release and acts on how it was set up.
//!
//! Installed (`install.json`, `pious-setup.exe` or `uninstall.exe` next to
//! the app): the setup program installed in the folder runs in update mode,
//! waits for Pious to close, downloads and verifies the release itself,
//! replaces the files in the install folder and starts Pious again. Only
//! older installs without `pious-setup.exe` need `Pious-Setup.exe` fetched
//! here, checked against the SHA-256 digest GitHub publishes.
//! Not installed: `pious.exe` is downloaded, verified and swapped in for the
//! running one (Windows won't let a running file be overwritten, but it can
//! be renamed, so the old one is moved aside and deleted on the next start).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use futures::SinkExt;
use futures::StreamExt;
use futures::channel::mpsc::Sender;
use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// The GitHub repository releases are published to (`owner/name`).
pub const REPOSITORY: &str = "woogi999/pious-bootstrapper";

/// The installer, fetched only by installs that don't have their own copy.
pub const SETUP_ASSET: &str = "Pious-Setup.exe";
/// The app alone, for copies that weren't installed.
pub const PORTABLE_ASSET: &str = "pious.exe";
/// Present on every release the installer can install.
pub const MANIFEST_ASSET: &str = "manifest.json";
/// The installer's copy of itself inside the install folder.
const SETUP_COPY: &str = "pious-setup.exe";

/// Whether `dir` (the folder holding the app) was set up by the installer.
fn marks_install(dir: &Path) -> bool {
    ["install.json", SETUP_COPY, "uninstall.exe"].iter().any(|name| dir.join(name).is_file())
}

/// Whether this copy was installed with the installer (it leaves
/// `install.json`, `pious-setup.exe` or `uninstall.exe` next to the app).
pub fn installed() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| Some(marks_install(exe.parent()?)))
        .unwrap_or(false)
}

/// The installer already in the install folder, if there is one.
fn own_setup() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(|d| d.join(SETUP_COPY)).filter(|p| p.is_file())
}

/// The asset that makes a release usable by this copy.
fn wanted_asset() -> &'static str {
    if installed() { MANIFEST_ASSET } else { PORTABLE_ASSET }
}

/// The version of this build.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Release {
    /// Version without the leading `v`, e.g. `0.2.0`.
    pub version: String,
    pub notes: String,
    pub page: String,
    #[serde(skip)]
    download: String,
    #[serde(skip)]
    size: u64,
    /// Expected SHA-256 of the asset, lowercase hex.
    #[serde(skip)]
    sha256: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Progress {
    Downloading { received: u64, total: u64 },
    /// The verified new executable, ready to be swapped in.
    Ready(PathBuf),
    Failed(String),
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    html_url: String,
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
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

fn client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent(concat!("Pious/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("HTTP client")
    })
}

/// Parses `v1.2.3` / `1.2.3` (ignoring any `-suffix`) into comparable parts.
fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let core = text.trim().trim_start_matches(['v', 'V']).split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    Some((parts.next()??, parts.next().flatten().unwrap_or(0), parts.next().flatten().unwrap_or(0)))
}

fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

/// The latest published release, if it's newer than this build.
pub async fn check() -> Result<Option<Release>, String> {
    let url = format!("https://api.github.com/repos/{REPOSITORY}/releases/latest");
    let response = client()
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| format!("Couldn't reach GitHub ({e})"))?;

    // No release has been published yet.
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(format!("GitHub answered {}", response.status()));
    }
    let release: ApiRelease = response
        .json()
        .await
        .map_err(|e| format!("Unexpected answer from GitHub ({e})"))?;

    if release.draft || release.prerelease || !is_newer(&release.tag_name, CURRENT) {
        return Ok(None);
    }
    let wanted = wanted_asset();
    if !release.assets.iter().any(|a| a.name.eq_ignore_ascii_case(wanted)) {
        return Ok(None);
    }
    // What this copy downloads itself: the app (portable), or the release's
    // installer. Installed copies take the new installer even though they
    // have one: it copies itself into the install folder as it updates, so
    // fixes to the installer reach everyone. (Their own copy is the fallback
    // if that download fails.)
    let fetched = if installed() { SETUP_ASSET } else { PORTABLE_ASSET };
    let asset = release.assets.into_iter().find(|a| a.name.eq_ignore_ascii_case(fetched));

    Ok(Some(Release {
        version: release.tag_name.trim_start_matches(['v', 'V']).to_owned(),
        notes: release.body.unwrap_or_default(),
        page: release.html_url,
        download: asset.as_ref().map(|a| a.browser_download_url.clone()).unwrap_or_default(),
        size: asset.as_ref().map(|a| a.size).unwrap_or(0),
        sha256: asset.and_then(|a| digest_hex(a.digest.as_deref())),
    }))
}

/// `sha256:ABC...` -> `abc...`.
fn digest_hex(digest: Option<&str>) -> Option<String> {
    digest.and_then(|d| d.strip_prefix("sha256:")).map(str::to_ascii_lowercase)
}

fn current_exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("Couldn't find the running app ({e})"))
}

fn staged_path(exe: &Path) -> PathBuf {
    if installed() {
        std::env::temp_dir().join("Pious-Setup-update.exe")
    } else {
        exe.with_extension("update.exe")
    }
}

fn retired_path(exe: &Path) -> PathBuf {
    exe.with_extension("old.exe")
}

/// Removes leftovers of a previous update. Called at startup.
pub fn clean_up() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(retired_path(&exe));
        let _ = std::fs::remove_file(staged_path(&exe));
    }
}

/// Downloads and verifies a release, reporting progress.
pub async fn download(release: Release, mut events: Sender<Progress>) {
    let mut result = if release.download.is_empty() { Err("This release doesn't include the installer.".to_owned()) } else { fetch(&release, &mut events).await };
    // The new installer couldn't be fetched: the one in the install folder
    // downloads and verifies the app itself just the same.
    if result.is_err() && installed() {
        if let Some(setup) = own_setup() {
            result = Ok(setup);
        }
    }
    let _ = events
        .send(match result {
            Ok(path) => Progress::Ready(path),
            Err(error) => Progress::Failed(error),
        })
        .await;
}

async fn fetch(release: &Release, events: &mut Sender<Progress>) -> Result<PathBuf, String> {
    let staged = staged_path(&current_exe()?);
    if release.download.is_empty() {
        return Err("This release doesn't include the installer.".into());
    }

    let response = client()
        .get(&release.download)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| format!("Download failed ({e})"))?;
    let total = response.content_length().unwrap_or(release.size);

    let mut file = tokio::fs::File::create(&staged)
        .await
        .map_err(|e| format!("Couldn't write the update ({e})"))?;
    let mut hasher = Sha256::new();
    let mut received = 0u64;
    let mut stream = response.bytes_stream();
    let mut last_report = 0u64;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download interrupted ({e})"))?;
        hasher.update(&chunk);
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("Couldn't write the update ({e})"))?;
        received += chunk.len() as u64;
        // Report roughly every 256 KB so the UI isn't flooded.
        if received - last_report >= 256 * 1024 || received == total {
            last_report = received;
            let _ = events.send(Progress::Downloading { received, total }).await;
        }
    }
    tokio::io::AsyncWriteExt::flush(&mut file)
        .await
        .map_err(|e| format!("Couldn't write the update ({e})"))?;
    drop(file);

    let digest: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if let Some(expected) = &release.sha256 {
        if *expected != digest {
            let _ = tokio::fs::remove_file(&staged).await;
            return Err("The download didn't match the published checksum, so it wasn't installed.".into());
        }
    }
    if received < 1024 * 1024 {
        let _ = tokio::fs::remove_file(&staged).await;
        return Err("The download was incomplete.".into());
    }
    Ok(staged)
}

/// Passed to the new version so it waits for the old one to close.
pub const AFTER_UPDATE: &str = "--after-update";

/// Swaps the verified executable in and starts it. The caller exits right
/// after; if anything fails the original stays in place.
pub fn install_and_restart(staged: &Path) -> Result<(), String> {
    let exe = current_exe()?;
    if installed() {
        // The installer waits for this process to close, then updates. It
        // runs as administrator (its manifest says so), which a plain
        // spawn can't start: ShellExecute asks Windows, which shows the
        // usual permission prompt.
        let folder = exe.parent().ok_or("Couldn't find the install folder.")?;
        let args = format!("--update --dir \"{}\" --wait {}", folder.display(), std::process::id());
        return run_as_admin(staged, &args);
    }
    let retired = retired_path(&exe);
    let _ = std::fs::remove_file(&retired);

    std::fs::rename(&exe, &retired).map_err(|e| format!("Couldn't replace the app ({e})"))?;
    if let Err(error) = std::fs::rename(staged, &exe) {
        // Put the original back.
        let _ = std::fs::rename(&retired, &exe);
        return Err(format!("Couldn't replace the app ({error})"));
    }

    std::process::Command::new(&exe)
        .arg(AFTER_UPDATE)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Updated, but couldn't restart ({e}). Open Pious again."))
}

/// Starts `exe` with `args` as administrator (Windows asks first).
#[cfg(windows)]
fn run_as_admin(exe: &Path, args: &str) -> Result<(), String> {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{HSTRING, w};
    let (file, params) = (HSTRING::from(exe.as_os_str()), HSTRING::from(args));
    let result = unsafe { ShellExecuteW(None, w!("runas"), &file, &params, None, SW_SHOWNORMAL) };
    // Above 32 means it started.
    if result.0 as isize > 32 {
        Ok(())
    } else if result.0 as isize == 5 {
        Err("The update needs your permission to install. Try again and choose Yes.".into())
    } else {
        Err(format!("Couldn't start the update (error {}).", result.0 as isize))
    }
}

#[cfg(not(windows))]
fn run_as_admin(exe: &Path, args: &str) -> Result<(), String> {
    std::process::Command::new(exe).args(args.split(' ')).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_digests() {
        assert_eq!(digest_hex(Some("sha256:ABcd")).as_deref(), Some("abcd"));
        assert_eq!(digest_hex(Some("md5:abcd")), None);
        assert_eq!(digest_hex(None), None);
    }

    #[test]
    fn recognises_installed_folders() {
        let dir = std::env::temp_dir().join(format!("pious-updater-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!marks_install(&dir));
        for name in ["install.json", "pious-setup.exe", "uninstall.exe"] {
            let marker = dir.join(name);
            std::fs::write(&marker, b"x").unwrap();
            assert!(marks_install(&dir), "{name}");
            std::fs::remove_file(&marker).unwrap();
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn compares_versions() {
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0-beta", "0.1.0"));
        assert!(!is_newer("garbage", "0.1.0"));
    }
}
