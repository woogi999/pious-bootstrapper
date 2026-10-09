//! Which Roblox build Pious starts, and marking it as the build in use.
//!
//! Every launch Pious starts itself goes through a [`Bootstrapper`]. Its
//! resolver names the build: [`LiveResolver`] asks Roblox for LIVE's
//! current one, [`PinnedResolver`] hands back a stored GUID and never
//! touches the network. Installing a missing build stays in
//! `versions::install_from`, which checks every file against
//! [`FILE_MANIFEST`] with [`verify`] before the build is used.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use futures::future::BoxFuture;

use crate::core::roblox;

/// The only release channel Pious resolves. Roblox's rollout channels are
/// ignored: a build from one makes the client update itself (see
/// `core::channel`).
pub const LOCKED_CHANNEL: &str = "LIVE";

/// Roblox's list of every file in a build and its checksum, kept in the
/// build's folder.
pub const FILE_MANIFEST: &str = "rbxManifest.txt";
/// The build's GUID, written into its folder whenever it's selected.
pub const VERSION_MARKER: &str = "version.txt";
const EXECUTABLE: &str = "RobloxPlayerBeta.exe";

/// One Roblox client build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deployment {
    /// The full upload name, "version-1a2b3c4d5e6f7a8b". Assumption: "GUID"
    /// means this whole string everywhere (folder name, version.txt,
    /// registry), as Roblox's own installer and the CDN file names use it.
    pub version_guid: String,
    /// "0.742.0.7421053"; empty when not known (a pinned GUID).
    pub version: String,
}

/// Names the build to run on `channel`. Async (a boxed future, so it can sit
/// behind `dyn`) because the rest of Pious runs on tokio.
pub trait DeploymentResolver: Send + Sync {
    fn resolve<'a>(&'a self, channel: &'a str) -> BoxFuture<'a, Result<Deployment>>;
}

/// The build Roblox serves on a channel right now.
pub struct LiveResolver;

impl DeploymentResolver for LiveResolver {
    fn resolve<'a>(&'a self, channel: &'a str) -> BoxFuture<'a, Result<Deployment>> {
        Box::pin(async move {
            let channel: String = channel.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect();
            ensure!(!channel.is_empty(), "No release channel to ask Roblox about");
            let url = format!("https://clientsettingscdn.roblox.com/v2/client-version/WindowsPlayer/channel/{channel}");
            let body: serde_json::Value = roblox::http()
                .get(&url)
                .send()
                .await
                .context("Couldn't reach Roblox to find its current version")?
                .error_for_status()
                .context("Roblox didn't say what its current version is")?
                .json()
                .await
                .context("Roblox's answer about its current version was unreadable")?;
            parse_client_version(&body)
        })
    }
}

/// `{"version":"0.742.0.7421053","clientVersionUpload":"version-cec3ad5889b447cf",…}`
fn parse_client_version(body: &serde_json::Value) -> Result<Deployment> {
    let upload = body["clientVersionUpload"].as_str().context("Roblox didn't report a current version")?;
    Ok(Deployment { version_guid: normalize_guid(upload)?, version: body["version"].as_str().unwrap_or_default().to_owned() })
}

/// A build chosen by the operator: always that GUID, offline.
pub struct PinnedResolver {
    guid: String,
}

impl PinnedResolver {
    pub fn new(guid: &str) -> Result<Self> {
        Ok(Self { guid: normalize_guid(guid)? })
    }
}

impl DeploymentResolver for PinnedResolver {
    // A GUID names one build whatever the channel, so the channel is unused.
    fn resolve<'a>(&'a self, _channel: &'a str) -> BoxFuture<'a, Result<Deployment>> {
        Box::pin(async move { Ok(Deployment { version_guid: self.guid.clone(), version: String::new() }) })
    }
}

/// "1A2B…", " version-1a2b… " → "version-1a2b…"; anything that isn't 16 hex
/// digits (DeployHistory's "version-hidden"…) is refused.
pub fn normalize_guid(input: &str) -> Result<String> {
    let value = input.trim().to_ascii_lowercase();
    let hex = value.strip_prefix("version-").unwrap_or(&value);
    ensure!(hex.len() == 16 && hex.chars().all(|c| c.is_ascii_hexdigit()), "A version GUID looks like version-1a2b3c4d5e6f7a8b, not \"{}\"", input.trim());
    Ok(format!("version-{hex}"))
}

/// Picks the build for a launch and marks it as the one in use.
pub struct Bootstrapper {
    resolver: Box<dyn DeploymentResolver>,
    versions_dir: PathBuf,
}

impl Bootstrapper {
    pub fn new(resolver: Box<dyn DeploymentResolver>, versions_dir: PathBuf) -> Self {
        Self { resolver, versions_dir }
    }

    /// `chosen` (a profile's or a game's GUID) ?? `pinned` ?? LIVE's build.
    pub fn for_launch(chosen: Option<&str>, pinned: Option<&str>, versions_dir: PathBuf) -> Result<Self> {
        let resolver: Box<dyn DeploymentResolver> = match chosen.or(pinned) {
            Some(guid) => Box::new(PinnedResolver::new(guid)?),
            None => Box::new(LiveResolver),
        };
        Ok(Self::new(resolver, versions_dir))
    }

    pub async fn resolve(&self) -> Result<Deployment> {
        self.resolver.resolve(LOCKED_CHANNEL).await
    }

    /// `<versions_dir>\version-<GUID>`.
    pub fn target(&self, deployment: &Deployment) -> PathBuf {
        self.versions_dir.join(&deployment.version_guid)
    }

    /// Marks an installed build as the one in use: its version.txt and the
    /// registry values. Returns its executable.
    pub fn select(&self, deployment: &Deployment) -> Result<PathBuf> {
        let target = self.target(deployment);
        ensure!(is_complete(&target), "{} isn't fully installed in {}", deployment.version_guid, target.display());
        std::fs::write(target.join(VERSION_MARKER), &deployment.version_guid)
            .with_context(|| format!("Couldn't write {}", target.join(VERSION_MARKER).display()))?;
        write_registry(&deployment.version_guid)?;
        Ok(target.join(EXECUTABLE))
    }
}

/// Whether a build folder has everything a verified install leaves.
pub fn is_complete(dir: &Path) -> bool {
    [EXECUTABLE, FILE_MANIFEST, VERSION_MARKER].iter().all(|f| dir.join(f).is_file())
}

/// rbxManifest.txt, as served (checked against the CDN on 2026-10-10):
/// alternating lines, a path inside the build folder with backslashes, then
/// that file's MD5 as 32 lowercase hex digits. Assumption: there is no
/// SHA-256 form (`{GUID}-rbxManifest.sha256` answers 403), so files are
/// checked against these MD5s.
pub fn parse_file_manifest(text: &str) -> Result<Vec<(PathBuf, String)>> {
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    ensure!(!lines.is_empty() && lines.len() % 2 == 0, "{FILE_MANIFEST} is malformed ({} lines)", lines.len());
    lines
        .chunks(2)
        .map(|pair| {
            let (name, hash) = (pair[0], pair[1].to_ascii_lowercase());
            ensure!(hash.len() == 32 && hash.chars().all(|c| c.is_ascii_hexdigit()), "{FILE_MANIFEST} has a bad checksum for {name}");
            let path: PathBuf = name.split(['\\', '/']).collect();
            let inside = path.components().all(|c| matches!(c, Component::Normal(_)));
            ensure!(inside, "{FILE_MANIFEST} names a file outside the build: {name}");
            Ok((path, hash))
        })
        .collect()
}

/// Checks every file `manifest` lists in `dir` (blocking: it reads the whole
/// build). Fails naming the files that are missing or differ. Run once at
/// install: tweaks change some of these files afterwards on purpose.
pub fn verify(dir: &Path, manifest: &str) -> Result<usize> {
    let entries = parse_file_manifest(manifest)?;
    let mut bad = Vec::new();
    for (path, expected) in &entries {
        match md5_of(&dir.join(path)) {
            Ok(actual) if &actual == expected => {}
            Ok(actual) => bad.push(format!("{} (MD5 {actual}, expected {expected})", path.display())),
            Err(_) => bad.push(format!("{} (missing)", path.display())),
        }
    }
    if !bad.is_empty() {
        let shown: Vec<&str> = bad.iter().take(5).map(String::as_str).collect();
        bail!("{} of {} files failed verification against {FILE_MANIFEST}: {}", bad.len(), entries.len(), shown.join("; "));
    }
    Ok(entries.len())
}

fn md5_of(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut context = md5::Context::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        context.consume(&buffer[..read]);
    }
    Ok(format!("{:x}", context.compute()))
}

/// Writes the selected build's GUID to HKCU (created if needed) and to HKLM
/// only when a machine-wide Roblox install made that key: creating keys
/// there needs administrator rights Pious doesn't run with.
#[cfg(windows)]
pub fn write_registry(guid: &str) -> Result<()> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_SET_VALUE};

    const USER: &str = r"Software\Roblox\RobloxPlayer";
    const MACHINE: &str = r"Software\WOW6432Node\Roblox\RobloxPlayer";

    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(USER).with_context(|| format!(r"Couldn't open HKCU\{USER}"))?;
    key.set_value("Version", &guid).with_context(|| format!(r"Couldn't write HKCU\{USER}\Version"))?;
    match RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(MACHINE, KEY_SET_VALUE) {
        Ok(key) => key.set_value("Version", &guid).with_context(|| format!(r"Couldn't write HKLM\{MACHINE}\Version"))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e).with_context(|| format!(r"HKLM\{MACHINE} exists but Pious can't write to it")),
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn write_registry(_guid: &str) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guids_are_normalized_and_checked() -> Result<()> {
        assert_eq!(normalize_guid(" CEC3AD5889B447CF ")?, "version-cec3ad5889b447cf");
        assert_eq!(normalize_guid("version-cec3ad5889b447cf")?, "version-cec3ad5889b447cf");
        assert!(normalize_guid("version-hidden").is_err());
        assert!(normalize_guid("version-cec3ad5889b447c").is_err());
        Ok(())
    }

    #[test]
    fn reads_the_live_answer() -> Result<()> {
        let body = serde_json::json!({"version":"0.742.0.7421053","clientVersionUpload":"version-cec3ad5889b447cf","bootstrapperVersion":""});
        assert_eq!(parse_client_version(&body)?, Deployment { version_guid: "version-cec3ad5889b447cf".into(), version: "0.742.0.7421053".into() });
        Ok(())
    }

    #[tokio::test]
    async fn pinned_never_asks_roblox() -> Result<()> {
        let boot = Bootstrapper::for_launch(None, Some("cec3ad5889b447cf"), PathBuf::from("Versions"))?;
        assert_eq!(boot.resolve().await?.version_guid, "version-cec3ad5889b447cf");
        // A profile's or game's GUID comes before the pin.
        let boot = Bootstrapper::for_launch(Some("version-0123456789abcdef"), Some("cec3ad5889b447cf"), PathBuf::from("Versions"))?;
        assert_eq!(boot.target(&boot.resolve().await?), Path::new("Versions").join("version-0123456789abcdef"));
        Ok(())
    }

    #[test]
    fn verifies_files_against_the_manifest() -> Result<()> {
        let dir = std::env::temp_dir().join(format!("pious-verify-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("content").join("fonts"))?;
        std::fs::write(dir.join("a.txt"), b"hello")?;
        std::fs::write(dir.join("content").join("fonts").join("b.ttf"), b"")?;
        // MD5("hello"), MD5("").
        let good = "a.txt\n5d41402abc4b2a76b9719d911017c592\ncontent\\fonts\\b.ttf\nd41d8cd98f00b204e9800998ecf8427e\n";
        assert_eq!(verify(&dir, good)?, 2);
        let tampered = good.replace("5d41402a", "00000000");
        assert!(verify(&dir, &tampered).is_err());
        assert!(verify(&dir, "missing.dll\nd41d8cd98f00b204e9800998ecf8427e\n").is_err());
        assert!(parse_file_manifest("..\\evil.dll\nd41d8cd98f00b204e9800998ecf8427e\n").is_err());
        std::fs::remove_dir_all(&dir)?;
        Ok(())
    }

    /// Network: `cargo test --bin pious -- --ignored live_resolver`.
    #[tokio::test]
    #[ignore]
    async fn live_resolver_names_a_real_build() -> Result<()> {
        let live = LiveResolver.resolve(LOCKED_CHANNEL).await?;
        assert!(live.version_guid.starts_with("version-") && !live.version.is_empty(), "{live:?}");
        Ok(())
    }
}
