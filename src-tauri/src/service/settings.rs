//! Preferences, the window's look and behavior, and self-updates.

use futures::StreamExt;
use serde_json::Value;
use tauri::Manager;

use super::{Service, Shared, Tone, UpdateState};
use crate::core::model::{Appearance, Blur, Preferences};
use crate::core::{process, store, updater};

impl Service {
    /// Applies a partial preferences object from the window (only the keys
    /// it sends change), then whatever those changes affect.
    pub async fn update_preferences(self: &Shared, patch: Value) -> Result<(), String> {
        let (before, after) = self.mutate(|s| -> Result<(Preferences, Preferences), String> {
            let before = s.bootstrapper.preferences.clone();
            let mut merged = serde_json::to_value(&before).map_err(|e| e.to_string())?;
            merge(&mut merged, patch);
            let after: Preferences = serde_json::from_value(merged).map_err(|e| e.to_string())?;
            // Taking over links has its own command (it edits the registry).
            let mut after = after;
            after.handle_roblox_links = before.handle_roblox_links;
            after.previous_handlers = before.previous_handlers.clone();
            s.bootstrapper.preferences = after.clone();
            s.dirty = true;
            Ok((before, after))
        })?;

        if before.multi_instance != after.multi_instance {
            process::set_multi_instance(after.multi_instance);
        }
        let look = (&before.appearance, &after.appearance);
        if before.pinned != after.pinned
            || look.0.see_through != look.1.see_through
            || look.0.blur != look.1.blur
            || look.0.blur_strength != look.1.blur_strength
        {
            apply_window(self);
        }
        // Turned off, the page asks for the account it wants itself.
        if after.friends_all && !before.friends_all {
            let me = self.clone();
            tokio::spawn(async move { me.refresh_friends(None).await });
        }
        if before.discord_presence != after.discord_presence {
            if after.discord_presence {
                let other = self.read().bootstrappers.iter().find(|b| b.discord_presence).map(|b| b.name);
                if let Some(name) = other {
                    self.toast(
                        Tone::Neutral,
                        format!("{name} also shows Discord activity. Pious leaves games {name} started to it, so you won't see double."),
                    );
                }
            }
            self.sync_presence().await;
        }
        let keys = |p: &Preferences| {
            let r = &p.recorder;
            (
                p.overlay.enabled,
                p.overlay.hotkey.clone(),
                p.overlay.game_only,
                (r.recording, r.clips, r.game_only),
                [r.record_hotkey.clone(), r.clip_hotkey.clone(), r.manual_clip_hotkey.clone()],
            )
        };
        let automation = |p: &Preferences| {
            (
                p.plugins.contains(crate::core::model::MACROS_PLUGIN),
                p.macros.iter().map(|m| (m.id, m.hotkey.clone(), m.game_only)).collect::<Vec<_>>(),
                p.autoclicker.enabled,
                p.autoclicker.hotkey.clone(),
                p.autoclicker.game_only,
                p.macro_settings.record_hotkey.clone(),
                p.macro_settings.stop_hotkey.clone(),
            )
        };
        if before.keybinds != after.keybinds
            || before.emoji_shortcodes != after.emoji_shortcodes
            || before.input_overlay != after.input_overlay
            || before.plugins != after.plugins
        {
            self.read().input_dirty = true;
            self.input_settings_changed();
        }
        if before.mcp != after.mcp {
            self.restart_mcp();
        }
        if before.plugins != after.plugins {
            tokio::task::spawn_blocking(crate::service::plugins::refresh_sdk);
        }
        if keys(&before) != keys(&after) || automation(&before) != automation(&after) {
            self.apply_hotkeys(true);
            self.prepare_overlay();
        }
        if before.start_with_windows != after.start_with_windows || before.start_hidden != after.start_hidden {
            if let Err(error) = crate::platform::system::set_run_at_startup(after.start_with_windows, after.start_hidden) {
                self.toast(Tone::Negative, error);
            }
        }
        if before.discord_display != after.discord_display
            || before.discord_join != after.discord_join
            || before.studio_presence != after.studio_presence
            || before.pious_presence != after.pious_presence
            || before.discord_account != after.discord_account
        {
            self.sync_presence().await;
        }
        if before.tweaks != after.tweaks {
            // Check the FastFlags now rather than at the next launch.
            if let Err(error) = crate::core::tweaks::fast_flags(&after.tweaks) {
                self.toast(Tone::Caution, error);
            }
            if before.tweaks.enabled && !after.tweaks.enabled {
                self.restore_tweaks().await;
            }
        }
        if before.auto_update != after.auto_update && after.auto_update {
            self.check_for_update(false).await;
        }
        Ok(())
    }

    /// Undoes Pious's tweaks in every build not in use right now.
    pub async fn restore_tweaks(self: &Shared) {
        let builds: Vec<std::path::PathBuf> = {
            let s = self.read();
            s.bootstrapper
                .versions
                .iter()
                .filter(|v| !s.instances.iter().any(|i| i.version.as_deref() == Some(v.hash.as_str())))
                .map(|v| v.path.clone())
                .collect()
        };
        let failed = tokio::task::spawn_blocking(move || {
            let failed = builds.iter().filter(|b| crate::core::tweaks::restore(b).is_err()).count();
            crate::core::fscache::refresh(builds.iter().map(|b| crate::core::tweaks::applied_marker(b)));
            failed
        })
        .await
        .unwrap_or(0);
        if failed > 0 {
            self.toast(Tone::Caution, "Some builds still have tweaks because they're in use. They're undone next time.");
        }
        self.emit();
    }

    /// Lets the user choose a font file for the font tweak.
    pub async fn pick_font(self: &Shared) {
        let picked = rfd::AsyncFileDialog::new()
            .set_title("Choose a font")
            .add_filter("Fonts", &["ttf", "otf"])
            .pick_file()
            .await
            .map(|f| f.path().to_path_buf());
        let Some(path) = picked else { return };
        // Keep a copy with the library so the font survives the original moving.
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("ttf").to_ascii_lowercase();
        let target = store::data_dir().join(format!("font.{ext}"));
        let stored = match std::fs::copy(&path, &target) {
            Ok(_) => target,
            Err(_) => path,
        };
        self.mutate(|s| {
            s.bootstrapper.preferences.tweaks.font = Some(stored);
            s.bootstrapper.preferences.tweaks.font_preset = None;
            s.dirty = true;
        });
    }

    /// A font file the window can show a preview with.
    pub async fn font_preview(&self, preset: String) -> Result<std::path::PathBuf, String> {
        let cache = store::data_dir().join("cache");
        if let Some(file) = preset.strip_prefix("roblox:") {
            let file = std::path::Path::new(file).file_name().ok_or("That font doesn't exist.")?.to_owned();
            let target = cache.join("font-previews").join(&file);
            if target.is_file() {
                return Ok(target);
            }
            let builds: Vec<std::path::PathBuf> = self.read().bootstrapper.versions.iter().filter(|v| v.valid).map(|v| v.path.clone()).collect();
            for build in builds {
                // The original, even when a font tweak replaced it.
                let source = crate::core::tweaks::original(&build, &format!("content/fonts/{}", file.to_string_lossy()));
                if source.is_file() {
                    let _ = std::fs::create_dir_all(cache.join("font-previews"));
                    std::fs::copy(&source, &target).map_err(|e| e.to_string())?;
                    return Ok(target);
                }
            }
            return Err("Install a Roblox version to preview Roblox's fonts.".into());
        }
        let url = crate::core::fonts::preset_url(&preset).ok_or("That font doesn't exist.")?;
        let parts: Vec<&str> = url.rsplit('/').take(2).collect();
        let name = format!("{}-{}", parts[1], parts[0]);
        let mods = cache.join("mods");
        crate::core::fonts::cached_download(url, &mods, &name).await?;
        Ok(mods.join(name))
    }

    pub fn reset_appearance(self: &Shared) {
        self.mutate(|s| {
            s.bootstrapper.preferences.appearance = Appearance::default();
            s.dirty = true;
        });
        apply_window(self);
    }

    /// Lets the user choose a background picture. A copy is kept with the
    /// library so it survives the original moving.
    pub async fn pick_background(self: &Shared) {
        let picked = rfd::AsyncFileDialog::new()
            .set_title("Choose a background picture")
            .add_filter("Pictures", &["png", "jpg", "jpeg", "webp", "bmp", "gif"])
            .pick_file()
            .await
            .map(|f| f.path().to_path_buf());
        let Some(path) = picked else { return };
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("png").to_ascii_lowercase();
        // A new name each time, so the window doesn't show a cached old one.
        let target = store::data_dir().join(format!("background-{}.{ext}", chrono::Utc::now().timestamp()));
        let _ = std::fs::create_dir_all(store::data_dir());
        let stored = match std::fs::copy(&path, &target) {
            Ok(_) => target,
            Err(_) => {
                // Couldn't copy it: let the window load it where it is.
                use tauri::Manager;
                let _ = self.app().asset_protocol_scope().allow_file(&path);
                path
            }
        };
        self.mutate(|s| {
            if let Some(old) = s.bootstrapper.preferences.appearance.background_image.replace(stored) {
                if old.starts_with(store::data_dir()) {
                    let _ = std::fs::remove_file(old);
                }
            }
            s.dirty = true;
        });
    }

    // ── Updates ──────────────────────────────────────────────────────────

    /// Looks for a new release. Manual checks also report "up to date".
    pub async fn check_for_update(self: &Shared, manual: bool) {
        let busy = self.mutate(|s| {
            let busy = matches!(s.update, UpdateState::Checking | UpdateState::Downloading { .. } | UpdateState::Ready { .. });
            if !busy {
                s.update = UpdateState::Checking;
            }
            busy
        });
        if busy {
            return;
        }
        let result = updater::check().await;
        let toast = self.mutate(|s| match result {
            Ok(Some(release)) => {
                let message = format!("Pious {} is available", release.version);
                s.update = UpdateState::Available { release };
                Some((Tone::Active, message))
            }
            Ok(None) => {
                s.update = UpdateState::UpToDate;
                manual.then(|| (Tone::Positive, "You're on the latest version".to_owned()))
            }
            Err(error) => {
                s.update = UpdateState::Failed { error: error.clone() };
                // Quiet on startup: being offline isn't worth a warning.
                manual.then(|| (Tone::Caution, format!("Couldn't check for updates: {error}")))
            }
        });
        if let Some((tone, message)) = toast {
            self.toast(tone, message);
        }
    }

    pub async fn download_update(self: &Shared) {
        let release = self.mutate(|s| {
            let UpdateState::Available { release } = &s.update else { return None };
            let release = release.clone();
            s.update = UpdateState::Downloading { release: release.clone(), received: 0, total: 0 };
            Some(release)
        });
        let Some(release) = release else { return };
        let (sender, mut events) = futures::channel::mpsc::channel(16);
        tauri::async_runtime::spawn(updater::download(release.clone(), sender));
        while let Some(progress) = events.next().await {
            match progress {
                updater::Progress::Downloading { received, total } => {
                    let release = release.clone();
                    self.mutate_throttled(|s| s.update = UpdateState::Downloading { release, received, total });
                }
                updater::Progress::Ready(path) => {
                    let release = release.clone();
                    self.mutate(|s| s.update = UpdateState::Ready { release, path });
                    self.toast(Tone::Positive, "Update downloaded. Restart to finish.");
                }
                updater::Progress::Failed(error) => {
                    self.mutate(|s| s.update = UpdateState::Failed { error: error.clone() });
                    self.toast(Tone::Negative, error);
                }
            }
        }
    }

    pub async fn restart_to_update(self: &Shared) {
        let path = match &self.read().update {
            UpdateState::Ready { path, .. } => path.clone(),
            _ => return,
        };
        // Save first so the new version starts from the latest library.
        self.save_now().await;
        match updater::install_and_restart(&path) {
            Ok(()) => self.app().exit(0),
            Err(error) => {
                self.mutate(|s| s.update = UpdateState::Failed { error: error.clone() });
                self.toast(Tone::Negative, error);
            }
        }
    }
}

/// Deep-merges `patch` into `target` (objects key by key; anything else
/// replaces).
fn merge(target: &mut Value, patch: Value) {
    match (target, patch) {
        (Value::Object(target), Value::Object(patch)) => {
            for (key, value) in patch {
                merge(target.entry(key).or_insert(Value::Null), value);
            }
        }
        (target, patch) => *target = patch,
    }
}

/// Applies "keep on top" to the main window, and the see-through backdrop
/// to it and the chat window, so both look the same.
pub fn apply_window(service: &Service) {
    let (pinned, look) = {
        let s = service.read();
        (s.bootstrapper.preferences.pinned, s.bootstrapper.preferences.appearance.clone())
    };
    if let Some(main) = service.app().get_webview_window("main") {
        let _ = main.set_always_on_top(pinned);
    }
    for label in ["main", super::windows::CHAT_WINDOW] {
        if let Some(window) = service.app().get_webview_window(label) {
            apply_backdrop(&window, &look, label == "main");
        }
    }
}

/// The see-through backdrop for one window. Only the main window captures
/// what's behind it for the adjustable blur; others use Windows' frost.
fn apply_backdrop(window: &tauri::WebviewWindow, look: &crate::core::model::Appearance, captures: bool) {
    #[cfg(windows)]
    {
        let mut blur = if look.see_through { look.blur } else { Blur::Off };
        if blur == Blur::Adjustable && !captures {
            blur = Blur::Frosted;
        }
        let strength = look.blur_strength.clamp(0.0, 1.0);
        let target = window.clone();
        // The adjustable blur captures what's behind the window, so the
        // window itself must stay out of that capture.
        if let Ok(hwnd) = window.hwnd() {
            crate::core::process::exclude_from_capture(hwnd.0 as isize, blur == Blur::Adjustable);
        }
        let _ = window.run_on_main_thread(move || {
            let _ = window_vibrancy::clear_blur(&target);
            let _ = window_vibrancy::clear_acrylic(&target);
            let _ = window_vibrancy::clear_mica(&target);
            let _ = match blur {
                Blur::Off | Blur::Adjustable => Ok(()),
                // Windows sets the blur radius itself; the lower half of the
                // range uses its lighter blur, the upper half its heavier
                // acrylic, each with more frost as it goes up.
                Blur::Frosted if strength < 0.5 => {
                    let tint = (strength * 2.0 * 140.0) as u8;
                    window_vibrancy::apply_blur(&target, Some((10, 10, 12, tint)))
                }
                Blur::Frosted => {
                    let tint = (20.0 + (strength - 0.5) * 2.0 * 190.0) as u8;
                    window_vibrancy::apply_acrylic(&target, Some((10, 10, 12, tint)))
                }
                Blur::Diffused => window_vibrancy::apply_mica(&target, Some(true)),
            };
        });
    }
    #[cfg(not(windows))]
    let _ = (window, look, captures, Blur::Off);
}
