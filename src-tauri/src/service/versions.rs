//! Roblox versions: finding, installing and removing builds, weao.xyz's
//! build list and the bootstrappers on this computer.

use futures::StreamExt;

use super::{Install, Service, Shared, Tone};
use crate::core::deployment::{self, Bootstrapper, DeploymentResolver, LiveResolver};
use crate::core::model::{VersionChoice, VersionRecord};
use crate::core::{bootstrappers, roblox, store, versions};
use crate::platform::system;

impl Service {
    fn versions_dir(&self) -> std::path::PathBuf {
        self.read()
            .bootstrapper
            .preferences
            .versions_dir
            .clone()
            .unwrap_or_else(store::default_versions_dir)
    }

    pub async fn scan_versions(self: &Shared) {
        crate::core::fscache::clear();
        self.mutate(|s| s.scanning = true);
        let dir = self.versions_dir();
        let (found, latest) = futures::join!(versions::detect(dir), LiveResolver.resolve(deployment::LOCKED_CHANNEL));
        self.mutate(|s| {
            s.scanning = false;
            match latest {
                Ok(latest) => {
                    s.latest_error = None;
                    s.latest = Some(latest.into());
                }
                Err(error) => s.latest_error = Some(format!("{error:#}")),
            }
            merge_versions(s, found);
        });
    }

    pub async fn install_version(self: &Shared, hash: String, version: Option<String>) {
        self.install_version_with(hash, version, false).await;
    }

    /// The run path for every launch Pious starts itself: names the build
    /// (`chosen` ?? the pin ?? LIVE's current one), installs it if its
    /// folder isn't complete, then selects it (version.txt and registry).
    /// Returns its GUID and folder.
    pub(super) async fn prepare_build(self: &Shared, chosen: Option<String>) -> Result<(String, std::path::PathBuf), String> {
        let pinned = self.read().bootstrapper.preferences.pinned_version_guid.clone();
        let boot = Bootstrapper::for_launch(chosen.as_deref(), pinned.as_deref(), self.versions_dir()).map_err(|e| format!("{e:#}"))?;
        let deployment = boot.resolve().await.map_err(|e| format!("{e:#}"))?;
        let target = boot.target(&deployment);
        // Pinned and chosen builds skip this when installed: no update step.
        if !deployment::is_complete(&target) {
            let shown = if deployment.version.is_empty() { deployment.version_guid.clone() } else { deployment.version.clone() };
            self.toast(Tone::Active, format!("Installing Roblox {shown} first…"));
            let version = Some(deployment.version.clone()).filter(|v| !v.is_empty());
            if !self.install_version_with(deployment.version_guid.clone(), version, false).await {
                return Err(format!("Couldn't install {}, so it wasn't started.", deployment.version_guid));
            }
        }
        let guid = deployment.version_guid.clone();
        tokio::task::spawn_blocking(move || boot.select(&deployment))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("{e:#}"))?;
        Ok((guid, target))
    }

    /// Installs a build; `quiet` only says something when it worked (for
    /// background updates). Returns whether it's installed now.
    async fn install_version_with(self: &Shared, hash: String, version: Option<String>, quiet: bool) -> bool {
        let hash = hash.trim().to_owned();
        let started = self.mutate(|s| {
            if s.installs.contains_key(&hash) {
                return false;
            }
            let updating = s.bootstrapper.versions.iter().any(|v| v.is_usable() && v.hash != hash);
            let version = version.clone().or_else(|| s.latest.as_ref().filter(|l| l.hash == hash).map(|l| l.version.clone()));
            s.installs.insert(
                hash.clone(),
                Install { version, downloaded: 0, total: 0, stage: "Preparing".into(), updating },
            );
            true
        });
        if !started {
            return false;
        }

        let dir = self.versions_dir();
        let (sender, mut events) = futures::channel::mpsc::channel(64);
        let version = self.read().installs.get(&hash).and_then(|i| i.version.clone());
        tauri::async_runtime::spawn(versions::install_from(hash.clone(), version, dir, sender));
        while let Some(event) = events.next().await {
            match event {
                versions::InstallEvent::Progress { downloaded, total, stage } => {
                    if total > 0 {
                        super::taskbar::progress(self, Some(downloaded as f64 / total as f64));
                    }
                    self.mutate_throttled(|s| {
                        if let Some(install) = s.installs.get_mut(&hash) {
                            install.downloaded = downloaded;
                            install.total = total;
                            install.stage = stage;
                        }
                    });
                }
                versions::InstallEvent::Finished(result) => {
                    self.mutate(|s| {
                        s.installs.remove(&hash);
                        if let Ok(record) = &result {
                            s.bootstrapper.versions.retain(|v| v.hash != record.hash);
                            s.bootstrapper.versions.push(record.clone());
                            s.dirty = true;
                        }
                    });
                    super::taskbar::progress(self, None);
                    return match result {
                        Ok(record) => {
                            // A new build gets the tweaks straight away.
                            let me = self.clone();
                            tokio::spawn(async move { me.apply_tweaks_everywhere().await });
                            if quiet {
                                self.toast(Tone::Neutral, format!("Roblox was updated to {} in the background", record.title()));
                            } else {
                                self.toast(Tone::Positive, format!("{} is installed and ready", record.title()));
                            }
                            true
                        }
                        Err(error) => {
                            if !quiet {
                                self.toast(Tone::Negative, error);
                            }
                            false
                        }
                    };
                }
            }
        }
        false
    }

    /// Keeps Roblox up to date in the background when that's on: checks
    /// every half hour and installs a new build once. A build that failed
    /// to install isn't tried again for six hours.
    pub(super) async fn roblox_update_loop(self: Shared) {
        loop {
            self.update_roblox_in_background(false).await;
            tokio::time::sleep(std::time::Duration::from_secs(30 * 60)).await;
        }
    }

    pub async fn update_roblox_in_background(self: &Shared, now: bool) {
        use std::sync::Mutex;
        static FAILED: Mutex<Option<(String, std::time::Instant)>> = Mutex::new(None);
        if !self.read().bootstrapper.preferences.auto_update_roblox || self.read().bootstrapper.preferences.pinned_version_guid.is_some() {
            return;
        }
        if !now {
            // Give the start-up a moment; nothing here is urgent.
            tokio::time::sleep(std::time::Duration::from_secs(20)).await;
        }
        let Ok(latest) = LiveResolver.resolve(deployment::LOCKED_CHANNEL).await else { return };
        let latest: roblox::ClientVersion = latest.into();
        let installed = deployment::is_complete(&self.versions_dir().join(&latest.hash));
        let busy = {
            let s = self.read();
            !s.installs.is_empty() || s.roblox_updating
        };
        let recently_failed = FAILED.lock().ok().and_then(|f| f.clone()).is_some_and(|(hash, at)| hash == latest.hash && at.elapsed().as_secs() < 6 * 3600);
        if installed || busy || recently_failed {
            return;
        }
        self.mutate(|s| {
            s.roblox_updating = true;
            s.latest = Some(latest.clone());
        });
        let ok = self.install_version_with(latest.hash.clone(), Some(latest.version.clone()), true).await;
        self.mutate(|s| s.roblox_updating = false);
        if !ok {
            if let Ok(mut failed) = FAILED.lock() {
                *failed = Some((latest.hash, std::time::Instant::now()));
            }
        }
    }

    /// Validates a typed build hash and installs it.
    pub async fn install_hash(self: &Shared, input: String) -> Result<(), String> {
        let value = deployment::normalize_guid(&input).map_err(|e| e.to_string())?;
        if self.read().bootstrapper.version(&value).is_some_and(|v| v.is_usable()) {
            return Err("That version is already installed.".into());
        }
        self.spawn(move |s| async move { s.install_version(value, None).await });
        Ok(())
    }

    pub async fn remove_version(self: &Shared, hash: String) {
        let path = {
            let s = self.read();
            let Some(record) = s.bootstrapper.version(&hash) else { return };
            if s.instances.iter().any(|i| i.version.as_deref() == Some(hash.as_str())) {
                drop(s);
                self.toast(Tone::Caution, "Close the instances using this version first.");
                return;
            }
            record.path.clone()
        };
        let result = if path.exists() { versions::remove(path).await } else { Ok(()) };
        match result {
            Ok(()) => {
                self.mutate(|s| {
                    s.bootstrapper.versions.retain(|v| v.hash != hash);
                    let specific = VersionChoice::Specific(hash.clone());
                    if s.bootstrapper.preferences.default_version == specific {
                        s.bootstrapper.preferences.default_version = VersionChoice::Latest;
                    }
                    if s.bootstrapper.preferences.pinned_version_guid.as_deref() == Some(hash.as_str()) {
                        s.bootstrapper.preferences.pinned_version_guid = None;
                    }
                    for game in &mut s.bootstrapper.games {
                        if game.config.version == specific {
                            game.config.version = VersionChoice::Default;
                        }
                    }
                    s.dirty = true;
                });
                self.toast(Tone::Neutral, "Version removed");
            }
            Err(error) => self.toast(Tone::Negative, error),
        }
    }

    pub fn set_default_version(&self, choice: VersionChoice) {
        self.mutate(|s| {
            s.bootstrapper.preferences.default_version = choice;
            s.dirty = true;
        });
    }

    /// Pins LIVE's current build: asks Roblox for its GUID once, installs
    /// it if needed and pins that GUID.
    pub async fn pin_current_version(self: &Shared) -> Result<String, String> {
        let current = LiveResolver.resolve(deployment::LOCKED_CHANNEL).await.map_err(|e| format!("{e:#}"))?;
        let (guid, _) = self.prepare_build(Some(current.version_guid)).await?;
        self.pin_version(guid.clone()).await?;
        Ok(guid)
    }

    /// Pins a build by GUID (installed or not: the next launch installs it)
    /// and selects it, so the registry names it straight away when it's
    /// installed.
    pub async fn pin_version(self: &Shared, hash: String) -> Result<(), String> {
        let guid = deployment::normalize_guid(&hash).map_err(|e| e.to_string())?;
        self.mutate(|s| {
            s.bootstrapper.preferences.pinned_version_guid = Some(guid.clone());
            s.dirty = true;
        });
        if deployment::is_complete(&self.versions_dir().join(&guid)) {
            self.prepare_build(Some(guid)).await?;
        }
        Ok(())
    }

    pub fn unpin_version(&self) {
        self.mutate(|s| {
            s.bootstrapper.preferences.pinned_version_guid = None;
            s.dirty = true;
        });
    }

    /// Adds or repoints a named version profile.
    pub fn set_version_profile(&self, name: String, guid: String) -> Result<(), String> {
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Err("A version profile needs a name.".into());
        }
        let guid = deployment::normalize_guid(&guid).map_err(|e| e.to_string())?;
        self.mutate(|s| {
            s.bootstrapper.preferences.version_profiles.insert(name, guid);
            s.dirty = true;
        });
        Ok(())
    }

    /// Removes a version profile; games and the default that used it go
    /// back to the default version.
    pub fn remove_version_profile(&self, name: String) {
        self.mutate(|s| {
            s.bootstrapper.preferences.version_profiles.remove(&name);
            let profile = VersionChoice::Profile(name);
            if s.bootstrapper.preferences.default_version == profile {
                s.bootstrapper.preferences.default_version = VersionChoice::Latest;
            }
            for game in &mut s.bootstrapper.games {
                if game.config.version == profile {
                    game.config.version = VersionChoice::Default;
                }
            }
            s.dirty = true;
        });
    }

    pub async fn load_builds(self: &Shared) {
        let busy = self.mutate(|s| std::mem::replace(&mut s.builds_loading, true));
        if busy {
            return;
        }
        let result = roblox::weao_builds().await;
        self.mutate(|s| {
            s.builds_loading = false;
            match result {
                Ok(builds) => {
                    crate::core::cache::save("builds", &builds);
                    s.builds = builds;
                    s.builds_error = None;
                }
                Err(error) => s.builds_error = Some(error),
            }
        });
    }

    pub async fn refresh_bootstrappers(self: &Shared) {
        let found = tokio::task::spawn_blocking(bootstrappers::detect).await.unwrap_or_default();
        self.mutate(|s| {
            s.link_handler = system::link_handler("roblox-player");
            s.bootstrappers = found;
        });
        self.sync_presence().await;
    }
}

fn merge_versions(s: &mut super::State, found: Vec<VersionRecord>) {
    let latest = s.latest.clone();
    let mut merged = Vec::new();
    for mut record in found {
        if let Some(existing) = s.bootstrapper.version(&record.hash) {
            record.label = existing.label.clone();
            record.version = existing.version.clone().or(record.version);
            record.installed_at = existing.installed_at;
        }
        if let Some(latest) = &latest {
            if latest.hash == record.hash && record.version.is_none() {
                record.version = Some(latest.version.clone());
            }
        }
        merged.push(record);
    }
    // Keep versions whose files disappeared, so the user sees they're gone.
    for old in &s.bootstrapper.versions {
        if !merged.iter().any(|v| v.hash == old.hash) {
            let mut gone = old.clone();
            gone.valid = false;
            merged.push(gone);
        }
    }
    merged.sort_by(|a, b| b.installed_at.cmp(&a.installed_at));
    s.bootstrapper.versions = merged;
    s.dirty = true;
}
