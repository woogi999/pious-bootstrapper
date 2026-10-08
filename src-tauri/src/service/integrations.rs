//! Reaching outside the library: Roblox links from the browser and Discord
//! presence.

use std::time::Duration;

use super::{InstanceStatus, Service, Shared, Tone};
use crate::core::model::ServerChoice;
use crate::core::{bootstrappers, discord, process, roblox, store};
use crate::platform::system;

/// When Pious started, for how long it's shown on Discord.
static STARTED: std::sync::OnceLock<i64> = std::sync::OnceLock::new();
/// Avatar links of accounts shown on Discord, by user ID.
static AVATARS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<u64, String>>> = std::sync::LazyLock::new(Default::default);

/// Roblox Studio while it's open (for Discord).
#[derive(Debug, Clone, PartialEq)]
pub struct Studio {
    /// The place being edited, when its window says.
    pub place: Option<String>,
    /// When it opened, as a Unix timestamp.
    pub since: i64,
}

/// "Obby - Roblox Studio" → "Obby". The start page has no place.
fn studio_place(title: &str) -> Option<String> {
    let place = title.trim().strip_suffix("Roblox Studio")?.trim_end().trim_end_matches(['-', '–']).trim();
    (!place.is_empty()).then(|| place.to_owned())
}

impl Service {
    // ── Roblox links ─────────────────────────────────────────────────────

    /// Turns "launch all Roblox through Pious" on or off. Whatever opened
    /// Roblox links before (Roblox itself or a bootstrapper) is remembered
    /// and handed back when it's turned off.
    pub async fn set_handle_links(self: &Shared, on: bool) {
        if on {
            if let Err(error) = self.take_over_links() {
                self.toast(Tone::Negative, error);
                return;
            }
            let (has_version, owner) = {
                let s = self.read();
                (
                    s.bootstrapper.versions.iter().any(|v| v.is_usable()),
                    bootstrappers::link_owner(&s.bootstrappers).map(|b| b.name),
                )
            };
            if !has_version {
                self.toast(Tone::Caution, "Install a Roblox version in Versions so Pious can start games from roblox.com.");
            } else if let Some(owner) = owner {
                self.toast(
                    Tone::Neutral,
                    format!("Pious now opens Roblox links instead of {owner}. Turn this off to hand them back."),
                );
            } else {
                self.toast(Tone::Positive, "Pious now opens every Roblox launch, including Play on roblox.com.");
            }
        } else {
            let previous = self.mutate(|s| std::mem::take(&mut s.bootstrapper.preferences.previous_handlers));
            for scheme in system::ROBLOX_SCHEMES {
                let before = previous.iter().find(|(s, _)| s == scheme).map(|(_, c)| c.as_str());
                if let Err(error) = system::restore_link_handler(scheme, before) {
                    self.toast(Tone::Negative, error);
                }
            }
            self.toast(Tone::Neutral, "Roblox links are opened by whatever opened them before.");
        }
        self.mutate(|s| {
            s.bootstrapper.preferences.handle_roblox_links = on;
            s.dirty = true;
        });
        self.refresh_bootstrappers().await;
    }

    /// Chooses what opens Roblox links (Play on roblox.com): `"pious"`,
    /// `"roblox"` (the newest build installed by Roblox itself) or a
    /// bootstrapper by name.
    pub async fn set_link_handler(self: &Shared, target: String) {
        if target == "pious" {
            return self.set_handle_links(true).await;
        }
        let command = {
            let s = self.read();
            if target == "roblox" {
                s.bootstrapper
                    .versions
                    .iter()
                    .filter(|v| v.is_usable() && v.source == crate::core::model::VersionSource::Roblox)
                    .max_by_key(|v| v.installed_at)
                    .map(|v| format!("\"{}\" %1", v.executable().display()))
            } else {
                s.bootstrappers
                    .iter()
                    .find(|b| b.name == target)
                    .map(|b| format!("\"{}\" -player \"%1\"", b.exe.display()))
            }
        };
        let Some(command) = command else {
            self.toast(Tone::Negative, "That program isn't installed anymore.");
            return;
        };
        for scheme in system::ROBLOX_SCHEMES {
            if let Err(error) = system::set_link_command(scheme, &command) {
                self.toast(Tone::Negative, error);
                return;
            }
        }
        self.mutate(|s| {
            s.bootstrapper.preferences.handle_roblox_links = false;
            s.bootstrapper.preferences.previous_handlers.clear();
            s.dirty = true;
        });
        let name = if target == "roblox" { "Roblox".to_owned() } else { target };
        self.toast(Tone::Positive, format!("Roblox links now open with {name}."));
        self.refresh_bootstrappers().await;
    }

    fn take_over_links(&self) -> Result<(), String> {
        for scheme in system::ROBLOX_SCHEMES {
            if let Some(current) = system::link_handler(scheme).filter(|c| !system::is_pious_command(c)) {
                self.mutate(|s| {
                    let previous = &mut s.bootstrapper.preferences.previous_handlers;
                    previous.retain(|(s, _)| s != scheme);
                    previous.push((scheme.to_owned(), current));
                    s.dirty = true;
                });
            }
            system::register_link_handler(scheme)?;
        }
        Ok(())
    }

    /// At start-up: if a bootstrapper took Roblox links back while the
    /// option is on, take them again (remembering it as the one to restore).
    pub(super) fn reclaim_links(&self) {
        if !self.read().bootstrapper.preferences.handle_roblox_links {
            return;
        }
        let ours = system::link_handler("roblox-player").is_some_and(|c| system::is_pious_command(&c));
        if !ours {
            if let Err(error) = self.take_over_links() {
                self.toast(Tone::Caution, format!("Couldn't take over Roblox links again: {error}"));
            }
        }
    }

    /// Launches Roblox links that arrived from the browser.
    pub async fn check_links(self: &Shared) {
        for link in store::take_links().await {
            self.open_link(link).await;
        }
    }

    // ── Programs Pious watches for ───────────────────────────────────────

    /// Watches for Roblox Studio (for Discord) and streaming software (for
    /// automatic streamer mode).
    pub(super) async fn apps_loop(self: Shared) {
        // Streamer mode was turned on here, so it's turned off here too
        // (never when the user turned it on themselves).
        let mut auto_on = false;
        loop {
            tokio::time::sleep(Duration::from_secs(4)).await;
            let (studio_wanted, auto, apps, streamer) = {
                let s = self.read();
                let p = &s.bootstrapper.preferences;
                (p.discord_presence && p.studio_presence, p.streamer_auto, p.streamer_apps.clone(), p.streamer_mode)
            };
            if !studio_wanted && !auto {
                if self.read().studio.is_some() {
                    self.mutate(|s| s.studio = None);
                    self.sync_presence().await;
                }
                auto_on = false;
                continue;
            }
            let found = tokio::task::spawn_blocking(move || {
                let processes = process::processes();
                let streaming = processes.iter().any(|(_, name)| apps.iter().any(|a| a.trim().eq_ignore_ascii_case(name)));
                let studio = studio_wanted
                    .then(|| processes.iter().find(|(_, n)| n.eq_ignore_ascii_case("RobloxStudioBeta.exe")).map(|(pid, _)| *pid))
                    .flatten()
                    .map(|pid| process::window_of(pid).map(process::window_title).and_then(|t| studio_place(&t)));
                (streaming, studio)
            })
            .await;
            let Ok((streaming, studio)) = found else { continue };

            let changed = self.mutate(|s| {
                let next = studio.map(|place| Studio {
                    since: s.studio.as_ref().map(|old| old.since).unwrap_or_else(|| chrono::Utc::now().timestamp()),
                    place,
                });
                let changed = s.studio != next;
                s.studio = next;
                changed
            });
            if changed {
                self.sync_presence().await;
            }

            if auto && streaming && !streamer {
                auto_on = true;
                self.mutate(|s| {
                    s.bootstrapper.preferences.streamer_mode = true;
                    s.dirty = true;
                });
                self.toast(Tone::Neutral, "Streaming software is open, so streamer mode is on.");
            } else if auto && !streaming && streamer && auto_on {
                auto_on = false;
                self.mutate(|s| {
                    s.bootstrapper.preferences.streamer_mode = false;
                    s.dirty = true;
                });
                self.toast(Tone::Neutral, "Streaming software closed, so streamer mode is off.");
            } else if !streamer {
                auto_on = false;
            }
        }
    }

    // ── Discord ──────────────────────────────────────────────────────────

    /// Shows the most recently started game on Discord, unless presence is
    /// off or the bootstrapper that started it already shows it.
    pub async fn sync_presence(self: &Shared) {
        let wanted = {
            let s = self.read();
            if !s.bootstrapper.preferences.discord_presence {
                None
            } else {
                let bootstrapper_shows = s.bootstrappers.iter().any(|b| b.opens_links && b.discord_presence);
                s.instances
                    .iter()
                    .filter(|i| i.status != InstanceStatus::Launching)
                    .filter(|i| !(i.via_handler && bootstrapper_shows))
                    .max_by_key(|i| i.started)
                    .and_then(|i| Some((i.clone(), s.bootstrapper.game(i.game)?.clone())))
            }
        };
        let (display, join, studio, idle) = {
            let s = self.read();
            let p = &s.bootstrapper.preferences;
            // Pious itself, while nothing's being played.
            // Off means off: nothing at all, Pious included.
            let idle = (p.discord_presence && p.pious_presence && s.instances.is_empty()).then(|| {
                let games = s.bootstrapper.games.iter().filter(|g| g.in_library).count();
                discord::Activity {
                    details: "Picking a game".into(),
                    state: match games {
                        0 => "In the launcher".into(),
                        1 => "1 game in the library".into(),
                        n => format!("{n} games in the library"),
                    },
                    started: *STARTED.get_or_init(|| chrono::Utc::now().timestamp()),
                    image: None,
                    place_id: 0,
                    display: 0,
                    join_url: None,
                    button: Some(("Get Pious".into(), format!("https://github.com/{}", crate::core::updater::REPOSITORY))),
                    small: None,
                }
            });
            let studio = s.studio.clone().filter(|_| p.discord_presence);
            (p.discord_display, p.discord_join, studio, idle)
        };
        let display = if display == crate::core::model::DiscordDisplay::GameName { 2 } else { 0 };
        let Some((instance, game)) = wanted else {
            // No game: Roblox Studio, if it's open and wanted, else Pious.
            discord::set(
                studio
                    .map(|studio| discord::Activity {
                        details: match &studio.place {
                            Some(place) => format!("Editing {place}"),
                            None => "Roblox Studio".into(),
                        },
                        state: "In Roblox Studio".into(),
                        started: studio.since,
                        image: None,
                        place_id: 0,
                        display,
                        join_url: None,
                        button: None,
                        small: None,
                    })
                    .or(idle),
            );
            return;
        };

        let image = match game.universe_id {
            Some(universe) => {
                let cached = self.read().icon_urls.get(&universe).cloned();
                match cached {
                    Some(url) => Some(url),
                    None => match roblox::game_icon_url(universe).await {
                        Ok(url) => {
                            self.read().icon_urls.insert(universe, url.clone());
                            Some(url)
                        }
                        Err(_) => None,
                    },
                }
            }
            None => None,
        };
        let state = match instance.server {
            ServerChoice::Private(_) => "In a private server",
            ServerChoice::Public if instance.job.is_some() => "In a public server",
            ServerChoice::Public => "Playing",
        };
        // Only public servers can be joined by anyone with the link.
        let join_url = match (&instance.server, &instance.job) {
            (ServerChoice::Public, Some(job)) if join => Some(discord::join_link(game.place_id, job)),
            _ => None,
        };
        // The account playing, when wanted: its avatar in the corner.
        let account = {
            let s = self.read();
            if s.bootstrapper.preferences.discord_account {
                instance
                    .account
                    .and_then(|id| s.bootstrapper.accounts.iter().find(|a| a.id == id))
                    .map(|a| (a.user_id, a.username.clone(), a.display_name.clone()))
            } else {
                None
            }
        };
        let small = match account {
            Some((user_id, username, display_name)) => {
                let cached = AVATARS.lock().ok().and_then(|m| m.get(&user_id).cloned());
                let url = match cached {
                    Some(url) => Some(url),
                    None => {
                        let url = roblox::avatar_url(user_id).await.ok();
                        if let (Some(url), Ok(mut m)) = (&url, AVATARS.lock()) {
                            m.insert(user_id, url.clone());
                        }
                        url
                    }
                };
                let name = if display_name == username { format!("@{username}") } else { format!("{display_name} (@{username})") };
                url.map(|url| (url, format!("Playing as {name}")))
            }
            None => None,
        };
        discord::set(Some(discord::Activity {
            details: game.name.clone(),
            state: state.to_owned(),
            started: instance.started.timestamp(),
            image,
            place_id: game.place_id,
            display,
            join_url,
            button: None,
            small,
        }));
    }

    /// Tells Roblox the active account is online every minute while Pious
    /// is open (what the Roblox app does), so friends see you as online.
    pub(super) async fn online_loop(self: Shared) {
        loop {
            let wanted = {
                let s = self.read();
                let in_game = !s.instances.is_empty();
                (s.bootstrapper.preferences.appear_online && !in_game).then_some(s.bootstrapper.active_account).flatten()
            };
            if let Some(account) = wanted {
                if let Ok(token) = crate::core::credentials::load_session(account) {
                    let _ = roblox::register_presence(&token).await;
                }
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }
}
