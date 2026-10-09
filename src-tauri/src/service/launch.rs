//! Starting Roblox: Play, private servers, server hops, joining players and
//! links opened from the browser.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Instance, InstanceStatus, Service, Shared, Tone};
use crate::core::launcher::{self, LaunchError, LaunchRequest, WebLaunch};
use crate::core::model::{Activity, ServerChoice, VersionChoice, parse_server_link};
use crate::core::roblox::{self, GameInfo, Presence, UserInfo};
use crate::core::{credentials, process};

/// Everything needed to start a game.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LaunchPlan {
    pub game: Uuid,
    pub account: Option<Uuid>,
    pub version: VersionChoice,
    pub server: ServerChoice,
    /// A specific public server to join.
    #[serde(default)]
    pub job: Option<String>,
    /// Save the version and server as this game's setup.
    #[serde(default)]
    pub remember: bool,
    /// Also pin the account to this game (only when chosen on purpose).
    #[serde(default)]
    pub pin_account: bool,
    /// Skip the "already running" checks.
    #[serde(default)]
    pub force: bool,
    /// A private server link to join directly, without saving it.
    #[serde(default)]
    pub link: Option<String>,
    /// Where to join a public server: "auto", "best_ping" or a region ID
    /// (see `core::region`). `None` uses the setting.
    #[serde(default)]
    pub region: Option<String>,
}

/// What happened when asked to launch.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LaunchOutcome {
    Started,
    /// Nothing happened; the reason was shown as a toast.
    Stopped,
    /// Ask first, then launch again with `force`.
    Confirm { title: String, body: String, confirm: String },
    /// The account must sign in again before it can play.
    SignIn { account: Uuid },
    /// There are no accounts yet.
    AddAccount,
    /// Couldn't launch, for this reason. Shown as a toast and reported to
    /// the window as `Stopped`.
    #[serde(skip)]
    Blocked(String),
}

/// A player looked up by username.
#[derive(Debug, Clone, Serialize)]
pub struct PlayerFound {
    pub user: UserInfo,
    pub presence: Presence,
    pub game: Option<GameInfo>,
}

impl Service {
    /// Play with the game's setup (sidebar account unless one is pinned).
    pub fn play(self: &Shared, game: Uuid, server: Option<ServerChoice>, force: bool) -> LaunchOutcome {
        let plan = {
            let s = self.read();
            let Some(g) = s.bootstrapper.game(game) else { return LaunchOutcome::Stopped };
            let config = s.bootstrapper.effective_config(g);
            LaunchPlan {
                game,
                account: config.account,
                version: config.version,
                server: server.unwrap_or(config.server),
                job: None,
                remember: s.bootstrapper.preferences.remember_last_used,
                pin_account: false,
                force,
                link: None,
                region: None,
            }
        };
        self.launch(plan)
    }

    pub fn join_server(self: &Shared, server: Uuid, force: bool) -> LaunchOutcome {
        let game = self.read().bootstrapper.server(server).map(|s| s.game_id);
        match game {
            Some(game) => self.play(game, Some(ServerChoice::Private(server)), force),
            None => LaunchOutcome::Stopped,
        }
    }

    pub fn launch(self: &Shared, plan: LaunchPlan) -> LaunchOutcome {
        let prepared = self.mutate(|s| -> Result<Prepared, LaunchOutcome> {
            let library = &s.bootstrapper;
            let game = library.game(plan.game).cloned().ok_or(LaunchOutcome::Stopped)?;
            let Some(account) = plan
                .account
                .and_then(|id| library.account(id))
                .or_else(|| library.play_account())
                .cloned()
            else {
                return Err(LaunchOutcome::AddAccount);
            };
            if account.needs_sign_in {
                return Err(LaunchOutcome::SignIn { account: account.id });
            }

            let version = library.resolve_version(&plan.version).cloned();
            // The build the operator chose (profile, game, pin), installed by
            // the run path if it's missing; `None` is LIVE's current one.
            let chosen = library.chosen_guid(&plan.version).map_err(stop)?;

            if !plan.force {
                if let Some(running) = s.instances.iter().find(|i| i.account == Some(account.id)) {
                    let other = library.game(running.game).map(|g| g.name.clone()).unwrap_or_default();
                    return Err(LaunchOutcome::Confirm {
                        title: format!("Switch to {}?", game.name),
                        body: format!(
                            "{} is playing {other}. Roblox allows one game per account, so its window will switch to {} and reopen in the same spot.",
                            account.label(),
                            game.name
                        ),
                        confirm: "Switch game".into(),
                    });
                }
                if !library.preferences.multi_instance && !s.instances.is_empty() {
                    return Err(LaunchOutcome::Confirm {
                        title: "Multi-instance is off".into(),
                        body: "Another Roblox instance is running. With multi-instance off, Roblox will replace it. You can turn on multi-instance in Settings.".into(),
                        confirm: "Launch anyway".into(),
                    });
                }
            }

            // Start through the chosen bootstrapper, if it's still installed.
            let via = library
                .preferences
                .launch_via
                .as_deref()
                .and_then(|name| s.bootstrappers.iter().find(|b| b.name == name))
                .map(|b| b.exe.clone());
            // Pious starts the client itself (through `prepare_build`) unless
            // a bootstrapper does, or Roblox's own handler does because
            // nothing is installed, chosen, or meant to be handled by Pious.
            let run_path = via.is_none() && (version.is_some() || chosen.is_some() || library.preferences.handle_roblox_links);
            // Pious's tweaks go on the build it starts itself, also when another
            // window runs that build (Roblox only reads these files as it
            // needs them; a file that's in use is reported, not skipped).
            let tweaks = run_path.then(|| (version.as_ref().map(|v| v.path.clone()).unwrap_or_default(), library.preferences.tweaks.clone()));

            let direct = match &plan.link {
                Some(link) => Some(parse_server_link(link).ok_or_else(|| stop("That isn't a private server link."))?),
                None => None,
            };
            let server_link = match plan.server {
                _ if direct.is_some() => direct,
                ServerChoice::Public => None,
                ServerChoice::Private(id) => match library.server(id) {
                    Some(server) => Some(parse_server_link(&server.link).ok_or_else(|| {
                        stop(format!("The link for {} isn't a valid private server link.", server.name))
                    })?),
                    None => None,
                },
            };

            // Switching games on a busy account: the window it has now gives
            // way, and the new one opens in the same place.
            let mut placement = None;
            if let Some(old) = s.instances.iter().find(|i| i.account == Some(account.id)).cloned() {
                if let Some(pid) = old.pid {
                    placement = process::placement(pid);
                    let _ = process::terminate(pid);
                }
                if let Some(watch) = s.watches.get_mut(&old.id) {
                    watch.closing = true;
                }
                s.instances.retain(|i| i.id != old.id);
            }
            let settings_for = Some(account.custom_settings.then_some(account.id));
            // A public server picked by region or ping (not when joining
            // someone, a link or a private server).
            let region = (server_link.is_none() && plan.job.is_none())
                .then(|| plan.region.clone().unwrap_or_else(|| library.preferences.region.clone()))
                .filter(|r| !r.is_empty() && r != "auto");
            let behavior = library.preferences.on_game_launch;

            // Remember the choice and update recents.
            let now = s.now();
            let remember = plan.remember && plan.job.is_none();
            if let Some(g) = s.bootstrapper.game_mut(game.id) {
                g.last_played = Some(now);
                g.play_count += 1;
                if remember {
                    g.config.version = plan.version.clone();
                    g.config.server = plan.server;
                }
                // The account sticks to the game only when picked for it.
                if plan.pin_account {
                    g.config.account = Some(account.id);
                    g.config.pin_account = true;
                }
            }
            if let Some(a) = s.bootstrapper.account_mut(account.id) {
                a.last_used = Some(now);
            }
            if let ServerChoice::Private(id) = plan.server {
                if let Some(server) = s.bootstrapper.server_mut(id) {
                    server.last_joined = Some(now);
                }
            }
            s.bootstrapper.record_activity(Activity {
                at: now,
                game_id: game.id,
                account_id: Some(account.id),
                server: plan.server,
                version: version.as_ref().map(|v| v.hash.clone()),
            });
            s.dirty = true;

            let id = Uuid::new_v4();
            crate::core::stats::record(
                "launch",
                serde_json::json!({ "game": game.name, "place_id": game.place_id, "account": account.user_id, "private": matches!(plan.server, ServerChoice::Private(_)) }),
            );
            s.instances.push(Instance {
                id,
                pid: None,
                account: Some(account.id),
                game: game.id,
                server: plan.server,
                job: plan.job.clone(),
                version: version.as_ref().map(|v| v.hash.clone()),
                started: now,
                status: InstanceStatus::Launching,
                location: None,
                via_handler: !run_path,
                plan: Some(LaunchPlan { account: Some(account.id), ..plan.clone() }),
            });
            s.watches.insert(id, super::watch::Watch::new());
            process::set_multi_instance(s.bootstrapper.preferences.multi_instance);
            Ok((
                LaunchRequest {
                    account: account.id,
                    place_id: game.place_id,
                    server: server_link,
                    job: plan.job.clone(),
                    executable: version.as_ref().map(|v| v.executable()),
                    via: via.clone(),
                },
                id,
                {
                    let mut message = match (&via, s.bootstrapper.preferences.launch_via.as_deref()) {
                        (Some(_), Some(through)) => format!("Launching {} as {} through {through}", game.name, account.label()),
                        _ => format!("Launching {} as {}", game.name, account.label()),
                    };
                    // Saved to this game, over the account picked in the
                    // sidebar: say so, or it looks like the wrong account.
                    let pinned = game.config.pin_account && game.config.account == Some(account.id) && plan.account == Some(account.id);
                    let picked = s.bootstrapper.play_account().map(|a| a.id);
                    if pinned && picked.is_some_and(|p| p != account.id) {
                        message.push_str(&format!(
                            ". This game is set to always play as {}: change that in its Launch Configuration, or use Play with… for one game.",
                            account.label()
                        ));
                    }
                    message
                },
                tweaks,
                Extras {
                    region,
                    place_id: game.place_id,
                    account: account.id,
                    placement,
                    settings_for,
                    behavior,
                    run_path,
                    installed: version.as_ref().map(|v| (v.hash.clone(), v.path.clone())),
                    chosen,
                },
            ))
        });

        let (request, id, message, tweaks, extras) = match prepared {
            Ok(prepared) => prepared,
            Err(LaunchOutcome::Blocked(reason)) => {
                self.toast(Tone::Negative, reason);
                return LaunchOutcome::Stopped;
            }
            Err(outcome) => return outcome,
        };
        self.toast(Tone::Active, message);
        self.spawn(move |s| async move {
            let (mut request, mut tweaks) = (request, tweaks);
            if extras.run_path {
                let (guid, build) = match s.build_for_launch(extras.chosen.clone(), extras.installed.clone()).await {
                    Ok(build) => build,
                    Err(error) => {
                        s.launched(id, Err(LaunchError::Other(error))).await;
                        return;
                    }
                };
                request.executable = Some(build.join("RobloxPlayerBeta.exe"));
                if let Some((path, _)) = tweaks.as_mut() {
                    *path = build;
                }
                s.mutate(|st| {
                    if let Some(instance) = st.instances.iter_mut().find(|i| i.id == id) {
                        instance.version = Some(guid);
                    }
                });
            }
            // Tweaks kept in Roblox's settings file (the frame rate cap…).
            let settings_tweaks = tweaks.as_ref().map(|(_, t)| t.clone());
            if let Some((build, tweaks)) = tweaks {
                s.apply_tweaks_to(build, tweaks).await;
            }
            // The account's own Roblox settings (or the PC's).
            if let Some(account) = extras.settings_for {
                let profiles = crate::core::robloxsettings::Profiles::new(&crate::core::store::data_dir());
                if account.is_some() || profiles.current().is_some() {
                    let loaded = tokio::task::spawn_blocking(move || profiles.activate(account)).await;
                    if let Ok(Err(error)) = loaded {
                        s.toast(Tone::Caution, error);
                    }
                }
            }
            // After the account's settings are in place, so they aren't
            // swapped back out.
            // Also puts back values a tweak no longer wants (or all of them,
            // with tweaks off).
            if let Some(settings_tweaks) = settings_tweaks {
                let applied = tokio::task::spawn_blocking(move || {
                    crate::core::tweaks::apply_settings(&settings_tweaks, &crate::core::store::data_dir())
                })
                .await;
                if let Ok(Err(error)) = applied {
                    s.toast(Tone::Caution, error);
                }
            }
            if let Some(choice) = extras.region.clone() {
                s.pick_region_server(&choice, extras.account, extras.place_id, id, &mut request).await;
            }
            // Tell the client it's on LIVE (the locked channel), so it doesn't
            // update itself to the rollout channel Roblox put the account on.
            let _ = tokio::task::spawn_blocking(crate::core::channel::force_live).await;
            let result = launcher::launch(request).await;
            if let (Ok(launched), Some(placement)) = (&result, extras.placement) {
                if let Some(pid) = launched.pid {
                    tauri::async_runtime::spawn(async move {
                        // Wait for the new window, then put it where the old one was.
                        for _ in 0..40 {
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            if process::window_of(pid).is_some() {
                                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                                process::place(pid, placement);
                                break;
                            }
                        }
                    });
                }
            }
            if result.is_ok() {
                s.after_game_started(extras.behavior);
            }
            s.launched(id, result).await;
        });
        LaunchOutcome::Started
    }

    /// `prepare_build`, except that with nothing chosen and LIVE's build out
    /// of reach (offline…), the build already `installed` is played. The
    /// error is for the user.
    async fn build_for_launch(
        self: &Shared,
        chosen: Option<String>,
        installed: Option<(String, std::path::PathBuf)>,
    ) -> Result<(String, std::path::PathBuf), String> {
        match (self.prepare_build(chosen.clone()).await, installed) {
            (Ok(build), _) => Ok(build),
            (Err(error), Some(installed)) if chosen.is_none() => {
                self.toast(Tone::Caution, format!("{error} Playing the installed {} instead.", installed.0));
                Ok(installed)
            }
            (Err(error), _) => Err(error),
        }
    }

    async fn launched(self: &Shared, id: Uuid, result: Result<launcher::Launched, LaunchError>) {
        match result {
            Ok(launched) => {
                let tracked = self.mutate(|s| match s.instances.iter_mut().find(|i| i.id == id) {
                    Some(instance) => {
                        instance.pid = launched.pid;
                        // To tell a crash from a normal close later.
                        if let Some(pid) = launched.pid {
                            process::keep_exit_code(pid);
                        }
                        instance.status =
                            if launched.pid.is_some() { InstanceStatus::Running } else { InstanceStatus::Untracked };
                        true
                    }
                    None => false,
                });
                // Closed while it was still starting: close the window it opened.
                if !tracked {
                    if let Some(pid) = launched.pid {
                        let _ = tokio::task::spawn_blocking(move || process::terminate(pid)).await;
                    }
                    return;
                }
                if let Some(pid) = launched.pid {
                    self.watch_for_self_update(pid);
                }
                self.sync_presence().await;
            }
            Err(error) => {
                self.mutate(|s| {
                    let account = s.instances.iter().find(|i| i.id == id).and_then(|i| i.account);
                    s.instances.retain(|i| i.id != id);
                    if let (LaunchError::SignInRequired(_), Some(account)) = (&error, account) {
                        if let Some(account) = s.bootstrapper.account_mut(account) {
                            account.needs_sign_in = true;
                            s.dirty = true;
                        }
                    }
                });
                self.toast(Tone::Negative, error.message().to_owned());
            }
        }
    }

    /// Roblox can still decide a build is out of date (a release a moment
    /// ago): the client closes, Roblox's installer runs, and the game comes
    /// back signed in as whoever is signed in to Roblox's own app, not the
    /// account Pious started. If that happens within the first minute, say
    /// what's going on and fetch the update, so the next Play is right.
    fn watch_for_self_update(self: &Shared, pid: u32) {
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            for _ in 0..60 {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                let (alive, installer) = tokio::task::spawn_blocking(move || {
                    let installer = ["RobloxPlayerInstaller.exe", "RobloxPlayerLauncher.exe"].iter().any(|n| !process::pids_named(n).is_empty());
                    (process::is_alive(pid), installer)
                })
                .await
                .unwrap_or((true, false));
                if alive {
                    continue;
                }
                if installer {
                    me.toast(
                        Tone::Caution,
                        "Roblox closed to update itself. When its installer reopens the game, it signs in with the account saved in Roblox's own app, not the one you picked. Close that window and press Play again: Pious is getting the update.",
                    );
                    let _ = me.prepare_build(None).await;
                }
                return;
            }
        });
    }

    // ── Instances ────────────────────────────────────────────────────────

    pub async fn focus_instance(&self, id: Uuid) {
        let pid = self.read().instances.iter().find(|i| i.id == id).and_then(|i| i.pid);
        // Off the window's thread: a busy game can take a while to answer.
        let focused = match pid {
            Some(pid) => tokio::task::spawn_blocking(move || process::focus(pid)).await.unwrap_or(false),
            None => false,
        };
        if !focused {
            self.toast(Tone::Caution, "Couldn't bring that window to the front.");
        }
    }

    pub async fn close_instance(self: &Shared, id: Uuid) {
        let pid = self.mutate(|s| {
            if let Some(watch) = s.watches.get_mut(&id) {
                watch.closing = true;
            }
            s.instances.iter().find(|i| i.id == id).map(|i| i.pid)
        });
        let Some(pid) = pid else { return };
        if let Some(pid) = pid {
            let result = tokio::task::spawn_blocking(move || process::terminate(pid))
                .await
                .unwrap_or_else(|e| Err(e.to_string()));
            if let Err(error) = result {
                self.toast(Tone::Caution, error);
            }
        }
        self.mutate(|s| s.instances.retain(|i| i.id != id));
        self.sync_presence().await;
    }

    /// Auto arrange: lays every Roblox window (Pious's first, oldest first,
    /// then any others) out over the screens. Returns how many were moved.
    pub async fn arrange_windows(self: &Shared, layout: Option<String>) -> Result<usize, String> {
        use crate::core::arrange::{Rect, layout as plan};
        let (kind, mut pids) = {
            let s = self.read();
            let kind = layout.unwrap_or_else(|| s.bootstrapper.preferences.arrange_layout.clone());
            let mut ours: Vec<&Instance> = s.instances.iter().filter(|i| i.pid.is_some()).collect();
            ours.sort_by_key(|i| i.started);
            (kind, ours.into_iter().filter_map(|i| i.pid).collect::<Vec<u32>>())
        };
        let screens: Vec<Rect> = self
            .app()
            .available_monitors()
            .map_err(|e| e.to_string())?
            .iter()
            .map(|m| {
                let area = m.work_area();
                Rect { x: area.position.x, y: area.position.y, width: area.size.width as i32, height: area.size.height as i32 }
            })
            .collect();
        let moved = tokio::task::spawn_blocking(move || {
            for pid in process::roblox_pids() {
                if !pids.contains(&pid) {
                    pids.push(pid);
                }
            }
            pids.retain(|&pid| process::window_of(pid).is_some());
            let rects = plan(pids.len(), &screens, &kind);
            pids.iter().zip(rects).filter(|(pid, r)| process::move_to(**pid, r.x, r.y, r.width, r.height)).count()
        })
        .await
        .map_err(|e| e.to_string())?;
        if moved == 0 {
            return Err("There's no Roblox window to arrange.".into());
        }
        Ok(moved)
    }

    pub async fn close_all(self: &Shared) {
        let ids: Vec<Uuid> = self.read().instances.iter().map(|i| i.id).collect();
        for id in ids {
            self.close_instance(id).await;
        }
    }

    // ── Server hopping ───────────────────────────────────────────────────

    /// Joins a random other public server: of a game (with its usual
    /// account) or for a running instance (moving that window).
    pub async fn server_hop(self: &Shared, game: Option<Uuid>, instance: Option<Uuid>, force: bool) -> LaunchOutcome {
        crate::core::stats::record("server_hop", serde_json::json!({}));
        let key = match (game, instance) {
            (_, Some(id)) => format!("instance:{id}"),
            (Some(id), None) => format!("game:{id}"),
            _ => return LaunchOutcome::Stopped,
        };
        let target = {
            let mut s = self.read();
            let target = match instance {
                Some(id) => s.instances.iter().find(|i| i.id == id).map(|i| (i.game, i.account, i.job.clone())),
                None => game.map(|g| (g, None, None)),
            };
            let place = target.as_ref().and_then(|(g, ..)| s.bootstrapper.game(*g)).map(|g| g.place_id);
            match (target, place) {
                (Some(target), Some(place)) if s.hopping.insert(key.clone()) => Some((target, place)),
                _ => None,
            }
        };
        let Some(((game, account, current), place)) = target else { return LaunchOutcome::Stopped };
        self.emit();
        self.toast(Tone::Active, "Looking for another server…");

        let token = self.any_session(account).await;
        let servers = self.cached_servers(place, 1, token.as_deref()).await;
        self.mutate(|s| s.hopping.remove(&key));
        let servers = match servers {
            Ok(servers) => servers,
            Err(error) => {
                self.toast(Tone::Negative, error);
                return LaunchOutcome::Stopped;
            }
        };
        let choice = {
            let others: Vec<_> = servers.iter().filter(|s| Some(&s.job) != current.as_ref()).collect();
            crate::core::random::pick(&others).map(|s| s.job.clone())
        };
        let Some(job) = choice else {
            self.toast(Tone::Caution, "There's no other server with room right now.");
            return LaunchOutcome::Stopped;
        };

        // The old client leaves first: Roblox allows one session per account.
        if let Some(id) = instance {
            let pid = self.mutate(|s| {
                if let Some(watch) = s.watches.get_mut(&id) {
                    watch.closing = true;
                }
                s.instances.iter().find(|i| i.id == id).and_then(|i| i.pid)
            });
            if let Some(pid) = pid {
                let _ = process::terminate(pid);
            }
            self.mutate(|s| s.instances.retain(|i| i.id != id));
        }
        let config = {
            let s = self.read();
            s.bootstrapper.game(game).map(|g| s.bootstrapper.effective_config(g)).unwrap_or_default()
        };
        self.launch(LaunchPlan {
            game,
            account: account.or(config.account),
            version: config.version,
            server: ServerChoice::Public,
            job: Some(job),
            remember: false,
            pin_account: false,
            // Hopping a running window already closed it; hopping a game
            // asks first if its account is busy elsewhere.
            force: force || instance.is_some(),
            link: None,
            region: None,
        })
    }

    // ── Joining players ──────────────────────────────────────────────────

    /// Username → user ID → presence (asked as one of the user's accounts)
    /// → the server they're in. Works without being friends whenever the
    /// player lets everyone join them.
    pub async fn find_player(&self, username: String, account: Uuid) -> Result<PlayerFound, String> {
        let username = username.trim().trim_start_matches('@').to_owned();
        if username.is_empty() {
            return Err("Type a Roblox username.".into());
        }
        let user = roblox::user_by_name(&username).await?;
        let token = tokio::task::spawn_blocking(move || credentials::load_session(account))
            .await
            .map_err(|e| e.to_string())??;
        let presence = roblox::presence(&token, user.id).await.map_err(|e| {
            if e == "SESSION_EXPIRED" { "That account's session expired. Sign it in again.".to_owned() } else { e }
        })?;
        let game = match &presence {
            Presence::InGame { place_id: Some(place), .. } => roblox::fetch_game(*place).await.ok(),
            _ => None,
        };
        Ok(PlayerFound { user, presence, game })
    }

    /// Quick switch: `account` joins the server the game in front is in (the
    /// one the overlay opened over, else the newest), and the account that
    /// was playing there leaves.
    pub async fn quick_switch(self: &Shared, account: Uuid) -> LaunchOutcome {
        let (current, label) = {
            let s = self.read();
            let front = s.overlay_previous;
            let current = s
                .instances
                .iter()
                .find(|i| i.pid.and_then(process::window_of).is_some_and(|w| w == front))
                .or_else(|| s.instances.iter().filter(|i| i.account.is_some()).max_by_key(|i| i.started))
                .cloned();
            let label = s.bootstrapper.account(account).map(|a| a.label().to_owned()).unwrap_or_default();
            (current, label)
        };
        let Some(current) = current else {
            self.toast(Tone::Caution, "Start a game first, then switch who plays it.");
            return LaunchOutcome::Stopped;
        };
        if current.account == Some(account) {
            self.toast(Tone::Neutral, format!("{label} is already the one playing."));
            return LaunchOutcome::Stopped;
        }
        if current.server == ServerChoice::Public && current.job.is_none() {
            self.toast(Tone::Caution, "Pious doesn't know this server yet. Try again once the game has finished joining.");
            return LaunchOutcome::Stopped;
        }
        let version = current.plan.as_ref().map(|p| p.version.clone()).unwrap_or(VersionChoice::Default);
        let outcome = self.launch(LaunchPlan {
            game: current.game,
            account: Some(account),
            version,
            server: current.server.clone(),
            job: current.job.clone(),
            remember: false,
            pin_account: false,
            force: true,
            link: current.plan.as_ref().and_then(|p| p.link.clone()),
            region: None,
        });
        if matches!(outcome, LaunchOutcome::Started) {
            self.close_instance(current.id).await;
            self.toast(Tone::Positive, format!("Switched to {label} in the same server."));
        }
        outcome
    }

    pub fn join_player(
        self: &Shared,
        place: u64,
        job: Option<String>,
        account: Option<Uuid>,
        name: Option<String>,
        force: bool,
    ) -> LaunchOutcome {
        let game = self.game_for_place(place, name);
        self.launch(LaunchPlan {
            game,
            account,
            version: VersionChoice::Default,
            server: ServerChoice::Public,
            job,
            remember: false,
            pin_account: false,
            force,
            link: None,
            region: None,
        })
    }

    // ── Roblox links from the browser ────────────────────────────────────

    pub async fn open_link(self: &Shared, link: String) {
        if link == super::SHOW_MAIN {
            self.open_main(true);
            return;
        }
        // roblox.com's Play button: the browser's own session and ticket.
        if let Some(web) = WebLaunch::parse(&link) {
            let (installed, chosen) = {
                let s = self.read();
                let installed = s.bootstrapper.resolve_version(&VersionChoice::Default).map(|v| (v.hash.clone(), v.path.clone()));
                (installed, s.bootstrapper.chosen_guid(&VersionChoice::Default))
            };
            let chosen = match chosen {
                Ok(chosen) => chosen,
                Err(error) => {
                    self.toast(Tone::Negative, error);
                    return;
                }
            };
            let Some(place) = web.place_id() else {
                self.toast(Tone::Negative, "That Roblox link didn't say which game to open.");
                return;
            };
            let game = self.game_for_place(place, None);
            let id = Uuid::new_v4();
            self.mutate(|s| {
                let now = s.now();
                if let Some(g) = s.bootstrapper.game_mut(game) {
                    g.last_played = Some(now);
                    g.play_count += 1;
                }
                s.instances.push(Instance {
                    id,
                    pid: None,
                    account: None,
                    game,
                    server: ServerChoice::Public,
                    job: web.job_id(),
                    version: installed.as_ref().map(|(hash, _)| hash.clone()),
                    started: now,
                    status: InstanceStatus::Launching,
                    location: None,
                    via_handler: false,
                    plan: None,
                });
                s.watches.insert(id, super::watch::Watch::new());
                s.dirty = true;
                process::set_multi_instance(s.bootstrapper.preferences.multi_instance);
            });
            self.toast(Tone::Active, "Starting Roblox from your browser");
            let (guid, build) = match self.build_for_launch(chosen, installed).await {
                Ok(build) => build,
                Err(error) => {
                    self.launched(id, Err(LaunchError::Other(error))).await;
                    return;
                }
            };
            self.mutate(|s| {
                if let Some(instance) = s.instances.iter_mut().find(|i| i.id == id) {
                    instance.version = Some(guid);
                }
            });
            // Tweaks go on games started from the website too (they used to
            // apply only to games started from Pious's library).
            let tweaks = self.read().bootstrapper.preferences.tweaks.clone();
            self.apply_tweaks_to(build.clone(), tweaks.clone()).await;
            let _ = tokio::task::spawn_blocking(move || crate::core::tweaks::apply_settings(&tweaks, &crate::core::store::data_dir())).await;
            let _ = tokio::task::spawn_blocking(crate::core::channel::force_live).await;
            let result = launcher::launch_web(web, build.join("RobloxPlayerBeta.exe")).await;
            self.launched(id, result).await;
            return;
        }

        // roblox://experiences/start?placeId=…&gameInstanceId=…: play it as
        // the sidebar account.
        if let Ok(url) = url::Url::parse(link.trim().trim_matches('"')) {
            let query = |key: &str| {
                url.query_pairs()
                    .find(|(k, _)| k.eq_ignore_ascii_case(key))
                    .map(|(_, v)| v.into_owned())
            };
            if let Some(place) = query("placeId").and_then(|p| p.parse().ok()) {
                let job = query("gameInstanceId").filter(|j| !j.is_empty());
                // Pressing Play on the website is the confirmation, so the
                // "already playing" checks are skipped.
                match self.join_player(place, job, None, None, true) {
                    LaunchOutcome::SignIn { account } => {
                        let name = self.read().bootstrapper.account(account).map(|a| a.label().to_owned());
                        self.toast(
                            Tone::Caution,
                            format!("{} needs to sign in again before launching. Do that in Accounts.", name.unwrap_or_else(|| "That account".into())),
                        );
                    }
                    LaunchOutcome::AddAccount => self.toast(Tone::Caution, "Add an account in Pious to open Roblox links."),
                    _ => {}
                }
                return;
            }
        }
        self.toast(Tone::Caution, "Pious couldn't read that Roblox link.");
    }
}

/// A launch ready to start: the request, the new instance, the message to
/// show, the tweaks to put on its build first, and the rest.
type Prepared = (LaunchRequest, Uuid, String, Option<(std::path::PathBuf, crate::core::model::Tweaks)>, Extras);

/// What else a launch does around starting the client.
struct Extras {
    /// Join a server by region or ping ("best_ping" or a region ID).
    region: Option<String>,
    place_id: u64,
    account: Uuid,
    /// Where the window it replaces was.
    placement: Option<process::Placement>,
    /// Whose Roblox settings to load (`Some(None)` = the PC's own).
    settings_for: Option<Option<Uuid>>,
    behavior: crate::core::model::LaunchBehavior,
    /// Pious starts the client itself, through `prepare_build`.
    run_path: bool,
    /// The installed build the choice resolved to (GUID, folder): played
    /// when LIVE's build can't be reached.
    installed: Option<(String, std::path::PathBuf)>,
    /// The GUID the operator chose (profile, game, pin); `None` is LIVE's.
    chosen: Option<String>,
}

impl Service {
    /// Puts the tweaks (and enabled plugins' Roblox files) on a build right
    /// before it starts. Every launch path goes through here: games from
    /// the library, joins, links, and Play on roblox.com.
    pub(super) async fn apply_tweaks_to(&self, build: std::path::PathBuf, tweaks: crate::core::model::Tweaks) {
        let mods_dir = crate::core::tweaks::mods_dir(&crate::core::store::data_dir());
        let cache = crate::core::store::data_dir().join("cache").join("mods");
        let presets = match crate::core::tweaks::preset_mods(&tweaks, &cache, &build).await {
            Ok(presets) => presets,
            Err(error) => {
                self.toast(Tone::Caution, error);
                Vec::new()
            }
        };
        // Client files from enabled plugins (only while tweaks are on).
        let enabled = self.read().bootstrapper.preferences.plugins.clone();
        let on = tweaks.enabled;
        let plugin_files = tokio::task::spawn_blocking(move || if on { super::plugins::client_files(&enabled) } else { Vec::new() })
            .await
            .unwrap_or_default();
        let applied = tokio::task::spawn_blocking(move || {
            let applied = crate::core::tweaks::apply(&build, &tweaks, &mods_dir, presets, plugin_files);
            crate::core::fscache::refresh([crate::core::tweaks::applied_marker(&build)]);
            applied
        })
        .await;
        if let Ok(Err(error)) = applied {
            self.toast(Tone::Caution, format!("Tweaks weren't applied: {error}"));
        }
    }

    /// Finds the public server to join by region or ping (see
    /// `core::region`). Without one, Roblox picks as usual.
    async fn pick_region_server(&self, choice: &str, account: Uuid, place_id: u64, id: Uuid, request: &mut LaunchRequest) {
        let Ok(Ok(token)) = tokio::task::spawn_blocking(move || credentials::load_session(account)).await else { return };
        let found = tokio::time::timeout(std::time::Duration::from_secs(25), crate::core::region::pick(choice, &token, place_id)).await;
        match found {
            Ok(Ok((job, what))) => {
                request.job = Some(job.clone());
                self.mutate(|s| {
                    if let Some(instance) = s.instances.iter_mut().find(|i| i.id == id) {
                        instance.job = Some(job);
                    }
                });
                self.toast(Tone::Neutral, format!("Joining {what}"));
            }
            Ok(Err(why)) => self.toast(Tone::Caution, format!("Roblox picks the server: {why}.")),
            Err(_) => self.toast(Tone::Caution, "Finding a server took too long, so Roblox picks one."),
        }
    }

    /// Gets Pious out of the way once a game has started, if wanted.
    fn after_game_started(&self, behavior: crate::core::model::LaunchBehavior) {
        use crate::core::model::LaunchBehavior;
        use tauri::Manager;
        let Some(window) = self.app().get_webview_window("main") else { return };
        match behavior {
            LaunchBehavior::Stay => {}
            LaunchBehavior::Minimize => {
                let _ = window.minimize();
            }
            LaunchBehavior::HideUntilClosed => {
                let _ = window.hide();
                self.read().hidden_for_game = true;
            }
        }
    }

    /// Joins one specific public server of a game.
    pub fn join_job(self: &Shared, game: Uuid, job: String, force: bool) -> LaunchOutcome {
        let config = {
            let s = self.read();
            s.bootstrapper.game(game).map(|g| s.bootstrapper.effective_config(g)).unwrap_or_default()
        };
        self.launch(LaunchPlan {
            game,
            account: config.account,
            version: config.version,
            server: ServerChoice::Public,
            job: Some(job),
            remember: false,
            pin_account: false,
            force,
            link: None,
            region: None,
        })
    }

    /// Joins a private server straight from its link, without saving it.
    pub async fn join_link(self: &Shared, link: String, account: Option<Uuid>, force: bool) -> LaunchOutcome {
        let info = match self.server_link_info(link.clone()).await {
            Ok(info) => info,
            Err(error) => {
                self.toast(Tone::Negative, error);
                return LaunchOutcome::Stopped;
            }
        };
        let Some(place) = info.place_id else {
            self.toast(Tone::Caution, "That link doesn't say which game it's for. Save it with its game instead.");
            return LaunchOutcome::Stopped;
        };
        let game = self.game_for_place(place, info.game.clone());
        let config = {
            let s = self.read();
            s.bootstrapper.game(game).map(|g| s.bootstrapper.effective_config(g)).unwrap_or_default()
        };
        self.launch(LaunchPlan {
            game,
            account: account.or(config.account),
            version: config.version,
            server: ServerChoice::Public,
            job: None,
            remember: false,
            pin_account: false,
            force,
            link: Some(link),
            region: None,
        })
    }

    /// The game's public servers, for the server browser.
    pub async fn list_servers(&self, game: Uuid, refresh: bool) -> Result<Vec<roblox::PublicServer>, String> {
        let place = self.read().bootstrapper.game(game).map(|g| g.place_id).ok_or("That game isn't in Pious anymore.")?;
        if refresh {
            self.read().server_cache.remove(&place);
        }
        let token = self.any_session(None).await;
        self.cached_servers(place, 4, token.as_deref()).await
    }

    /// Server lists are cached for a little while: Roblox rate-limits them.
    async fn cached_servers(&self, place: u64, pages: usize, token: Option<&str>) -> Result<Vec<roblox::PublicServer>, String> {
        if let Some((at, cached_pages, servers)) = self.read().server_cache.get(&place) {
            if at.elapsed() < std::time::Duration::from_secs(30) && *cached_pages >= pages {
                return Ok(servers.clone());
            }
        }
        let servers = roblox::public_servers_paged(place, pages, token).await?;
        self.read().server_cache.insert(place, (std::time::Instant::now(), pages, servers.clone()));
        Ok(servers)
    }

    /// A signed-in session to make requests with: `prefer`'s, else the
    /// play-as account's, else any account's.
    pub async fn any_session(&self, prefer: Option<Uuid>) -> Option<String> {
        let candidates: Vec<Uuid> = {
            let s = self.read();
            prefer
                .into_iter()
                .chain(s.bootstrapper.active_account)
                .chain(s.bootstrapper.accounts.iter().map(|a| a.id))
                .collect()
        };
        for id in candidates {
            if let Ok(Ok(token)) = tokio::task::spawn_blocking(move || credentials::load_session(id)).await {
                return Some(token);
            }
        }
        None
    }
}

fn stop(message: impl Into<String>) -> LaunchOutcome {
    LaunchOutcome::Blocked(message.into())
}
