//! The backend of Pious: all state and logic behind the interface.
//!
//! The Svelte UI never owns data. It calls commands (see
//! [`crate::commands`]) and draws whatever the latest [`snapshot`] says.
//! Every change here pushes a fresh snapshot to the window as a `snapshot`
//! event, and short messages for the user go out as `toast` events.

mod accounts;
pub mod hotkeys;
pub mod input;
pub mod stats;
mod integrations;
mod launch;
mod library;
pub mod mcp;
mod news;
pub mod notify;
pub mod overlay;
pub mod pluginapi;
pub mod plugins;
pub mod recorder;
mod settings;
mod social;
pub mod taskbar;
pub mod themes;
mod versions;
mod watch;
pub mod windows;

pub use accounts::{BrowserSearch, QuickLoginUpdate};
pub use launch::{LaunchOutcome, LaunchPlan, PlayerFound};
pub use library::ServerForm;
pub use social::ServerLinkInfo;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::core::bootstrappers::InstalledBootstrapper;
use crate::core::model::{Bootstrapper, ServerChoice};
use crate::core::recommend::Recommendation;
use crate::core::roblox::{ClientVersion, KnownBuild, QuickLogin};
use crate::core::{process, store, updater};

/// A Roblox client started by Pious during this session.
#[derive(Debug, Clone, Serialize)]
pub struct Instance {
    pub id: Uuid,
    pub pid: Option<u32>,
    /// `None` for games started from roblox.com, which use the browser's
    /// own session.
    pub account: Option<Uuid>,
    pub game: Uuid,
    pub server: ServerChoice,
    /// The public server joined, when it was picked on purpose.
    pub job: Option<String>,
    pub version: Option<String>,
    pub started: DateTime<Utc>,
    pub status: InstanceStatus,
    /// Where its server is ("Ashburn, Virginia, US"), once known.
    pub location: Option<String>,
    /// Started through the system's Roblox handler (often a bootstrapper).
    #[serde(skip)]
    pub via_handler: bool,
    /// How it was launched, to launch it the same way when rejoining.
    /// `None` for games started from roblox.com.
    #[serde(skip)]
    pub plan: Option<launch::LaunchPlan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceStatus {
    Launching,
    Running,
    /// Started through the system handler; its process couldn't be identified.
    Untracked,
}

/// A version download in progress.
#[derive(Debug, Clone, Serialize)]
pub struct Install {
    pub version: Option<String>,
    pub downloaded: u64,
    pub total: u64,
    pub stage: String,
    pub updating: bool,
}

/// Where the app is with updating itself.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available { release: updater::Release },
    Downloading { release: updater::Release, received: u64, total: u64 },
    Ready {
        release: updater::Release,
        #[serde(skip)]
        path: std::path::PathBuf,
    },
    Failed { error: String },
}

/// The Roblox sign-in window, while it's open.
#[derive(Debug, Clone)]
pub struct SignIn {
    pub reauth: Option<Uuid>,
    pub verifying: bool,
}

#[derive(Default)]
pub struct State {
    pub bootstrapper: Bootstrapper,
    pub loaded: bool,
    pub load_error: Option<String>,
    /// The saved data on disk couldn't be read and must not be overwritten.
    pub save_blocked: bool,
    pub save_failing: bool,
    pub dirty: bool,

    pub instances: Vec<Instance>,
    pub installs: HashMap<String, Install>,
    pub latest: Option<ClientVersion>,
    pub latest_error: Option<String>,
    pub scanning: bool,

    pub recommendations: Vec<Recommendation>,
    pub recs_loading: bool,
    pub recs_error: Option<String>,
    /// FastFlags Roblox refused at the last launch (from its log).
    pub denied_flags: std::collections::BTreeSet<String>,

    pub bootstrappers: Vec<InstalledBootstrapper>,
    pub link_handler: Option<String>,
    pub builds: Vec<KnownBuild>,
    pub builds_loading: bool,
    pub builds_error: Option<String>,

    pub update: UpdateState,
    /// Downloaded artwork: cache key → file on disk.
    pub images: HashMap<String, String>,
    pub fetching: HashSet<String>,
    /// Server hops waiting for a server list ("game:<id>" / "instance:<id>").
    pub hopping: HashSet<String>,
    pub icon_urls: HashMap<u64, String>,
    pub quick_login: Option<QuickLogin>,
    pub sign_in: Option<SignIn>,
    /// Server locations already looked up, by address.
    pub server_locations: HashMap<String, String>,
    /// Logs, rejoin and anti-AFK state for each running window.
    pub watches: HashMap<Uuid, watch::Watch>,
    /// The window that had focus before the overlay opened.
    pub overlay_previous: isize,
    pub overlay_open: bool,
    /// Pious hid itself because a game started (it comes back when every
    /// Roblox window is closed).
    pub hidden_for_game: bool,
    /// Recently fetched public server lists, by place.
    pub server_cache: HashMap<u64, (Instant, usize, Vec<crate::core::roblox::PublicServer>)>,
    /// Registered global hotkeys, by shortcut ID.
    pub hotkeys: HashMap<u32, hotkeys::Action>,
    /// "Only in game" hotkeys, caught by the input hook: (keys, action).
    pub game_hotkeys: Vec<(String, hotkeys::Action)>,
    /// A Roblox window (or the overlay) is in front.
    pub in_game: bool,
    /// The Roblox window in front most recently (where background macros go).
    pub last_roblox_window: Option<isize>,
    /// The process of the Roblox window in front right now.
    pub foreground_roblox: Option<u32>,
    /// The input overlay's window.
    pub inputs: input::InputWindows,
    /// The stats overlay is being moved (it takes the mouse while it is).
    pub stats_editing: bool,
    /// The News page.
    pub news: news::News,
    /// Roblox Studio, while it's open and shown on Discord.
    pub studio: Option<integrations::Studio>,
    /// Settings the keyboard hook uses changed (apply them right away).
    pub input_dirty: bool,
    pub capture: recorder::Capture,
    pub friends: social::Friends,
    /// Roblox sign-ins found in browsers, waiting to be added.
    pub browser_sessions: Vec<accounts::BrowserSession>,
    /// The MCP server's address while it runs, and why it couldn't start.
    pub mcp: mcp::McpState,
    /// The startup logo finished (the main window may show).
    pub splash_done: bool,
    /// Found on disk by [`Service::disk_loop`] (never read while locked).
    pub plugins: Vec<plugins::Plugin>,
    pub themes: Vec<themes::Theme>,
    /// Stylesheets, icons and sounds from plugins and the theme in use.
    pub ui_assets: plugins::UiAssets,
    /// Crash reports kept in the data folder (newest first).
    pub crashes: Vec<crate::core::crash::Report>,
    /// Roblox is being updated in the background.
    pub roblox_updating: bool,
    /// Plugins' hotkeys, status chips, styles and input watching.
    pub plugin_runtime: pluginapi::Runtime,
    pub roblox_uninstallable: bool,
    /// Pious was installed (not run as a lone exe).
    pub installed: bool,
}

/// The locked state (see [`Service::read`]).
pub struct Guard<'a> {
    inner: MutexGuard<'a, State>,
    #[cfg(debug_assertions)]
    owner: &'a Mutex<Option<std::thread::ThreadId>>,
}

impl std::ops::Deref for Guard<'_> {
    type Target = State;
    fn deref(&self) -> &State {
        &self.inner
    }
}

impl std::ops::DerefMut for Guard<'_> {
    fn deref_mut(&mut self) -> &mut State {
        &mut self.inner
    }
}

#[cfg(debug_assertions)]
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        *self.owner.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// The shared backend, managed by Tauri.
pub struct Service {
    state: Mutex<State>,
    /// The thread holding `state` (debug builds), to catch re-locks.
    #[cfg(debug_assertions)]
    owner: Mutex<Option<std::thread::ThreadId>>,
    app: AppHandle,
    /// When the last snapshot went out, to keep fast progress updates from
    /// flooding the window.
    last_emit: Mutex<Instant>,
    /// A hash of the last snapshot sent, so unchanged state isn't resent
    /// (the window redraws on every snapshot).
    last_sent: Mutex<u64>,
    /// One save at a time, so an older copy never lands after a newer one.
    saving: tokio::sync::Mutex<()>,
}

pub type Shared = Arc<Service>;

/// Sent through the link inbox by a second copy of Pious: show this one.
pub const SHOW_MAIN: &str = "pious:show";

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Positive,
    Active,
    Caution,
    Negative,
    Neutral,
}

impl Service {
    pub fn new(app: AppHandle) -> Shared {
        Arc::new(Self {
            state: Mutex::new(State::default()),
            #[cfg(debug_assertions)]
            owner: Mutex::new(None),
            app,
            last_emit: Mutex::new(Instant::now()),
            last_sent: Mutex::new(0),
            saving: tokio::sync::Mutex::new(()),
        })
    }

    pub fn app(&self) -> &AppHandle {
        &self.app
    }

    /// Reads the state without telling the window anything changed.
    ///
    /// Never hold the guard across another `read()`, `mutate()` or `emit()`
    /// on the same thread (the lock isn't reentrant), nor across anything
    /// that waits on the main thread (window calls): either freezes Pious.
    /// In debug builds a re-lock on the same thread panics instead of
    /// hanging, so such a mistake shows up right away.
    pub fn read(&self) -> Guard<'_> {
        #[cfg(debug_assertions)]
        {
            if let Some(owner) = *self.owner.lock().unwrap_or_else(|e| e.into_inner()) {
                assert!(owner != std::thread::current().id(), "the state lock was taken twice on one thread (deadlock)");
            }
        }
        let guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        #[cfg(debug_assertions)]
        {
            *self.owner.lock().unwrap_or_else(|e| e.into_inner()) = Some(std::thread::current().id());
        }
        Guard { inner: guard, #[cfg(debug_assertions)] owner: &self.owner }
    }

    /// Changes the state, then sends the window a fresh snapshot.
    pub fn mutate<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        let result = f(&mut self.read());
        self.emit();
        result
    }

    /// Like [`Self::mutate`], but sends at most ~20 snapshots a second.
    /// For high-frequency progress updates.
    pub fn mutate_throttled(&self, f: impl FnOnce(&mut State)) {
        f(&mut self.read());
        let mut last = self.last_emit.lock().unwrap_or_else(|e| e.into_inner());
        if last.elapsed() >= Duration::from_millis(50) {
            *last = Instant::now();
            drop(last);
            self.emit();
        }
    }

    /// Sends the window the current state.
    pub fn emit(&self) {
        use std::hash::{Hash, Hasher};
        let text = snapshot(&self.read()).to_string();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        let hash = hasher.finish();
        {
            let mut last = self.last_sent.lock().unwrap_or_else(|e| e.into_inner());
            if *last == hash {
                return;
            }
            *last = hash;
        }
        if let Ok(raw) = serde_json::value::RawValue::from_string(text) {
            let _ = self.app.emit("snapshot", raw);
        }
    }

    pub fn toast(&self, tone: Tone, message: impl Into<String>) {
        let _ = self.app.emit("toast", json!({ "tone": tone, "message": message.into() }));
    }

    /// Loads the saved data, then starts the background work.
    pub async fn start(self: &Shared) {
        let loaded = store::load().await;
        let error = self.mutate(|s| {
            let error = match loaded {
                Ok(library) => {
                    s.bootstrapper = library;
                    None
                }
                Err(error) => {
                    if let Some(recovered) = error.recovered {
                        s.bootstrapper = *recovered;
                        // Saved (fixed) only once the original is backed up.
                        s.dirty = error.can_overwrite;
                    }
                    s.save_blocked = !error.can_overwrite;
                    s.load_error = Some(error.message.clone());
                    Some(error.message)
                }
            };
            s.bootstrapper.active_account = s.bootstrapper.default_account().map(|a| a.id);
            s.bootstrapper.preferences.tweaks.migrate();
            s.bootstrapper.preferences.migrate_defaults();
            s.loaded = true;
            s.images = store::cached_artwork();
            // Last session's lists, until fresh ones arrive.
            s.recommendations = crate::core::cache::load("recommendations").unwrap_or_default();
            s.denied_flags = crate::core::cache::load("denied-flags").unwrap_or_default();
            s.builds = crate::core::cache::load("builds").unwrap_or_default();
            s.news.items = crate::core::cache::load("news").unwrap_or_default();
            if let Some(account) = s.bootstrapper.active_account {
                s.friends.account = Some(account);
                s.friends.list = crate::core::cache::load(&format!("friends-{account}")).unwrap_or_default();
            }
            process::set_multi_instance(s.bootstrapper.preferences.multi_instance);
            error
        });
        if let Some(error) = error {
            self.toast(Tone::Negative, error);
        }

        // A background picture kept outside the data folder (it couldn't
        // be copied in) still needs to be allowed for the window.
        if let Some(picture) = self.read().bootstrapper.preferences.appearance.background_image.clone() {
            if !picture.starts_with(store::data_dir()) {
                use tauri::Manager;
                let _ = self.app().asset_protocol_scope().allow_file(&picture);
            }
        }
        self.reclaim_links();
        // Starting with Windows should start this copy (it may have moved).
        let (startup, hidden) = {
            let p = &self.read().bootstrapper.preferences;
            (p.start_with_windows, p.start_hidden)
        };
        if startup {
            let _ = crate::platform::system::set_run_at_startup(true, hidden);
        }
        settings::apply_window(self);
        self.apply_hotkeys(true);
        self.prepare_overlay();

        let me = self.clone();
        tauri::async_runtime::spawn(async move { me.save_loop().await });
        let me = self.clone();
        tauri::async_runtime::spawn(async move { me.poll_loop().await });
        let me = self.clone();
        tauri::async_runtime::spawn(async move { me.watch_loop().await });
        let me = self.clone();
        tauri::async_runtime::spawn(async move { me.anti_afk_loop().await });
        let me = self.clone();
        tauri::async_runtime::spawn(async move { me.housekeeping_loop().await });
        let me = self.clone();
        tauri::async_runtime::spawn(async move { me.backdrop_loop().await });
        let me = self.clone();
        tauri::async_runtime::spawn(async move { me.hotkey_focus_loop().await });
        let me = self.clone();
        tauri::async_runtime::spawn(async move { me.recorder_loop().await });
        let me = self.clone();
        tauri::async_runtime::spawn(async move { me.friends_loop().await });
        let me = self.clone();
        tauri::async_runtime::spawn(async move { me.disk_loop().await });
        let me = self.clone();
        tauri::async_runtime::spawn(async move { me.input_loop().await });
        let me = self.clone();
        tauri::async_runtime::spawn(async move { me.stats_loop().await });
        let me = self.clone();
        tauri::async_runtime::spawn(async move { me.apps_loop().await });
        let me = self.clone();
        tauri::async_runtime::spawn(async move { me.online_loop().await });
        let me = self.clone();
        tauri::async_runtime::spawn(async move { me.notify_loop().await });
        let me = self.clone();
        tauri::async_runtime::spawn(async move { me.roblox_update_loop().await });
        // Portable copies that used macros before they became a plugin
        // folder get the folder, then the window gets plugins' and the
        // theme's styles.
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            let _ = tokio::task::spawn_blocking(plugins::ensure_macros_plugin).await;
            // Macros kept in Pious's settings move into the Macros plugin.
            let legacy = {
                let s = me.read();
                let p = &s.bootstrapper.preferences;
                (serde_json::to_value(&p.macros).unwrap_or_default(), serde_json::to_value(&p.macro_settings).unwrap_or_default(), serde_json::to_value(&p.autoclicker).unwrap_or_default())
            };
            let moved = tokio::task::spawn_blocking(move || plugins::migrate_legacy_macros(&legacy.0, &legacy.1, &legacy.2)).await.unwrap_or(false);
            if moved {
                me.mutate(|s| {
                    let p = &mut s.bootstrapper.preferences;
                    p.macros = Default::default();
                    p.macro_settings = Default::default();
                    p.autoclicker = Default::default();
                    s.dirty = true;
                });
            }
            me.refresh_plugins().await;
            // Plugins' engines start (in a hidden window).
            let _ = tokio::task::spawn_blocking(plugins::refresh_sdk).await;
            me.sync_plugin_host();
            // Hotkeys were registered before the plugins folder was read:
            // macros' keys only exist once their plugin is known to be there.
            me.apply_hotkeys(true);
            me.emit_theme_assets();
            // Installed builds get the current tweaks (once the versions
            // have been looked at).
            tokio::time::sleep(Duration::from_secs(5)).await;
            me.apply_tweaks_everywhere().await;
        });
        // "Playing Pious" on Discord from the start.
        let me = self.clone();
        tauri::async_runtime::spawn(async move { me.sync_presence().await });
        self.restart_mcp();
        crate::core::stats::record("app_open", json!({ "version": updater::CURRENT }));

        let auto_update = self.read().bootstrapper.preferences.auto_update;
        let tasks = [
            self.spawn(|s| async move { s.scan_versions().await }),
            self.spawn(|s| async move { s.ensure_artwork().await }),
            self.spawn(|s| async move { s.refresh_recommendations().await }),
            self.spawn(|s| async move { s.refresh_bootstrappers().await }),
            self.spawn(|s| async move { s.check_links().await }),
            self.spawn(|s| async move { s.refresh_friends(None).await }),
            self.spawn(|s| async move { s.load_builds().await }),
            self.spawn(|s| async move { s.refresh_news(false).await }),
        ];
        drop(tasks);
        if auto_update {
            self.spawn(|s| async move { s.check_for_update(false).await });
        }
    }

    /// Runs `f` in the background with a handle to the service.
    pub fn spawn<F, Fut>(self: &Shared, f: F) -> tauri::async_runtime::JoinHandle<()>
    where
        F: FnOnce(Shared) -> Fut,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        tauri::async_runtime::spawn(f(self.clone()))
    }

    /// Writes the saved data a moment after it changes.
    async fn save_loop(self: Shared) {
        loop {
            tokio::time::sleep(Duration::from_millis(800)).await;
            let guard = self.saving.lock().await;
            let library = {
                let mut s = self.read();
                if !s.dirty || !s.loaded || s.save_blocked {
                    continue;
                }
                s.dirty = false;
                s.bootstrapper.clone()
            };
            let result = store::save(library).await;
            drop(guard);
            let report = {
                let mut s = self.read();
                match result {
                    Ok(()) => {
                        s.save_failing = false;
                        None
                    }
                    Err(error) => {
                        // Retried next time round; reported only once.
                        s.dirty = true;
                        (!std::mem::replace(&mut s.save_failing, true)).then_some(error)
                    }
                }
            };
            if let Some(error) = report {
                self.toast(Tone::Negative, format!("Couldn't save your settings: {error}"));
            }
        }
    }

    /// Saves right away (before exiting or restarting).
    pub async fn save_now(&self) {
        let _guard = self.saving.lock().await;
        let library = {
            let s = self.read();
            if !s.loaded || s.save_blocked {
                return;
            }
            s.bootstrapper.clone()
        };
        let _ = store::save(library).await;
    }

    /// Notices closed Roblox windows and Roblox links from the browser.
    async fn poll_loop(self: Shared) {
        let mut tick = 0u64;
        loop {
            tokio::time::sleep(Duration::from_millis(700)).await;
            tick += 1;

            // Links from the browser, and "show yourself" from a second copy.
            self.check_links().await;
            if tick % 2 != 0 {
                continue;
            }

            let exited: Vec<(Uuid, Uuid)> = {
                let s = self.read();
                s.instances
                    .iter()
                    .filter(|i| i.status == InstanceStatus::Running)
                    .filter(|i| i.pid.is_some_and(|pid| !process::is_alive(pid)))
                    .map(|i| (i.id, i.game))
                    .collect()
            };
            if exited.is_empty() {
                // Windows closed from Pious don't come through here: let go
                // of their kept handles every few seconds.
                if tick % 10 == 0 {
                    let watched: Vec<u32> = self.read().instances.iter().filter_map(|i| i.pid).collect();
                    process::prune_exit_codes(&watched);
                }
                continue;
            }
            self.report_crashes(&exited);
            // Crashed windows are rejoined (when that's on) instead.
            let mut closed = Vec::new();
            for (id, game) in exited {
                if !self.on_exited(id).await {
                    closed.push((id, game));
                }
            }
            let exited = closed;
            let settings: Vec<Option<Uuid>> = {
                let s = self.read();
                exited
                    .iter()
                    .filter_map(|(id, _)| s.instances.iter().find(|i| i.id == *id))
                    .map(|i| i.account.filter(|a| s.bootstrapper.account(*a).is_some_and(|a| a.custom_settings)))
                    .collect()
            };
            self.capture_roblox_settings(settings).await;
            let names: Vec<String> = self.mutate(|s| {
                let now = Utc::now();
                for (id, _) in &exited {
                    if let Some(i) = s.instances.iter().find(|i| i.id == *id) {
                        let game = s.bootstrapper.game(i.game);
                        crate::core::stats::record(
                            "session",
                            json!({
                                "game": game.map(|g| g.name.clone()),
                                "place_id": game.map(|g| g.place_id),
                                "account": i.account.and_then(|a| s.bootstrapper.account(a)).map(|a| a.user_id),
                                "started": i.started,
                                "seconds": (now - i.started).num_seconds().max(0),
                            }),
                        );
                    }
                }
                let names = exited
                    .iter()
                    .map(|(_, game)| s.bootstrapper.game(*game).map(|g| g.name.clone()).unwrap_or_else(|| "Roblox".into()))
                    .collect();
                s.instances.retain(|i| !exited.iter().any(|(id, _)| *id == i.id));
                s.watches.retain(|id, _| !exited.iter().any(|(e, _)| e == id));
                names
            });
            for name in names {
                self.toast(Tone::Neutral, format!("{name} was closed"));
            }
            self.sync_presence().await;
            self.show_after_games();
        }
    }
}

impl Service {
    /// Keeps what the window needs from the disk and registry fresh, away
    /// from the state lock: whether each Roblox build is still there and
    /// tweaked, the plugins folder, and whether Roblox can be uninstalled.
    async fn disk_loop(self: Shared) {
        let mut tick = 0u64;
        let mut last_assets = None;
        let mut last_macros = plugins::macros_present();
        // Plugins and themes are only read again when their folders change.
        let mut last_folders: Option<(u64, std::collections::BTreeSet<String>)> = None;
        loop {
            let (paths, enabled, theme) = {
                let s = self.read();
                let paths: Vec<std::path::PathBuf> =
                    s.bootstrapper.versions.iter().flat_map(|v| [v.executable(), crate::core::tweaks::applied_marker(&v.path)]).collect();
                let p = &s.bootstrapper.preferences;
                (paths, p.plugins.clone(), p.appearance.theme.clone())
            };
            // While a game is in front, look less often: nothing here is
            // urgent, and Roblox should have the PC to itself.
            let slow = self.read().in_game;
            let registry = tick % 10 == 0;
            let previous = last_folders.clone();
            let found = tokio::task::spawn_blocking(move || {
                crate::core::fscache::refresh(paths);
                // Unchanged folders and settings: nothing to read again.
                let key = (folders_signature(), enabled.clone());
                if previous.as_ref() == Some(&key) {
                    return None;
                }
                let plugins = plugins::list(&enabled);
                let themes = themes::list(&plugins);
                // Something about plugins or themes changed on disk: the
                // window's styles and icons may need to follow.
                let assets_key: Vec<_> = plugins
                    .iter()
                    .filter(|p| p.enabled)
                    .map(|p| (p.id.clone(), p.css.clone(), p.icons.len()))
                    .chain(themes.iter().filter(|t| Some(&t.id) == theme.as_ref()).map(|t| (t.id.clone(), t.css.clone(), t.icons.len())))
                    .collect();
                Some((key, plugins, themes, assets_key))
            })
            .await
            .ok()
            .flatten();
            // Pictures whose file is gone (the cache was cleared, by hand or
            // by a cleaner) are fetched again instead of showing nothing.
            let images: Vec<(String, String)> = if registry { self.read().images.iter().map(|(k, v)| (k.clone(), v.clone())).collect() } else { Vec::new() };
            let others = tokio::task::spawn_blocking(move || {
                let uninstallable = registry.then(|| crate::platform::system::uninstall_command("Roblox").is_some());
                let crashes = registry.then(crate::core::crash::list);
                let missing: Vec<String> = images.into_iter().filter(|(_, path)| !std::path::Path::new(path).is_file()).map(|(k, _)| k).collect();
                (uninstallable, crashes, updater::installed(), missing)
            })
            .await;
            if let Ok((_, _, _, missing)) = &others {
                if !missing.is_empty() {
                    self.mutate(|s| {
                        for key in missing {
                            s.images.remove(key);
                        }
                    });
                    let me = self.clone();
                    tauri::async_runtime::spawn(async move { me.ensure_artwork().await });
                }
            }
            let others = others.map(|(u, c, i, _)| (u, c, i));
            if let Ok((uninstallable, crashes, installed)) = others {
                self.mutate(|s| {
                    if let Some(u) = uninstallable {
                        s.roblox_uninstallable = u;
                    }
                    if let Some(c) = crashes {
                        s.crashes = c;
                    }
                    s.installed = installed;
                });
            }
            if let Some((key, plugins, themes, assets_key)) = found {
                last_folders = Some(key);
                self.mutate(|s| {
                    s.plugins = plugins;
                    s.themes = themes;
                });
                if last_assets.as_ref() != Some(&assets_key) {
                    last_assets = Some(assets_key);
                    self.emit_theme_assets();
                }
                // Engines follow the plugins on disk.
                let _ = tokio::task::spawn_blocking(plugins::refresh_sdk).await;
                self.sync_plugin_host();
                // The Macros plugin came or went: its hotkeys follow.
                if plugins::macros_present() != last_macros {
                    last_macros = plugins::macros_present();
                    self.apply_hotkeys(true);
                }
            }
            tick += 1;
            tokio::time::sleep(Duration::from_secs(if slow { 10 } else { 3 })).await;
        }
    }

    /// Writes a crash report for each closed window whose process ended
    /// with a crash code (when crash reports are on), and says so.
    fn report_crashes(self: &Shared, exited: &[(Uuid, Uuid)]) {
        let found: Vec<_> = {
            let s = self.read();
            // Read (and let go of) every exit code, wanted or not.
            let codes: Vec<(Uuid, Option<u32>)> =
                exited.iter().map(|(id, _)| (*id, s.instances.iter().find(|i| i.id == *id).and_then(|i| i.pid).and_then(process::exit_code))).collect();
            if !s.bootstrapper.preferences.crash_reports {
                return;
            }
            exited
                .iter()
                .filter_map(|(id, _)| {
                    let instance = s.instances.iter().find(|i| i.id == *id)?;
                    let code = codes.iter().find(|(c, _)| c == id).and_then(|(_, code)| *code)?;
                    if !crate::core::crash::is_crash_code(code) {
                        return None;
                    }
                    let game = s.bootstrapper.game(instance.game);
                    let log = s.watches.get(id).and_then(|w| w.log.as_ref()).map(|l| l.path.clone());
                    Some((
                        game.map(|g| g.name.clone()).unwrap_or_else(|| "Roblox".into()),
                        game.map(|g| g.place_id).unwrap_or_default(),
                        instance.version.clone(),
                        code,
                        log,
                        (Utc::now() - instance.started).num_minutes(),
                    ))
                })
                .collect()
        };
        for (game, place, version, code, log, minutes) in found {
            let me = self.clone();
            tauri::async_runtime::spawn(async move {
                let written = tokio::task::spawn_blocking(move || {
                    crate::core::crash::report_roblox(&game, place, version.as_deref(), code, log.as_deref(), minutes).map(|_| game)
                })
                .await
                .ok()
                .flatten();
                if let Some(game) = written {
                    let crashes = tokio::task::spawn_blocking(crate::core::crash::list).await.unwrap_or_default();
                    me.mutate(|s| s.crashes = crashes);
                    me.toast(Tone::Caution, format!("Roblox crashed in {game}. A crash report is in Settings → About."));
                }
            });
        }
    }

    /// Re-reads the plugins folder now (after one was made or toggled).
    pub async fn refresh_plugins(self: &Shared) {
        if let Ok(plugins) = tokio::task::spawn_blocking(|| plugins::list(&Default::default())).await {
            self.mutate(|s| s.plugins = plugins);
        }
    }

    /// Saves Roblox's settings file into the slots of the accounts whose
    /// windows just closed (`None` = the PC's own settings).
    pub async fn capture_roblox_settings(&self, accounts: Vec<Option<Uuid>>) {
        let profiles = crate::core::robloxsettings::Profiles::new(&store::data_dir());
        // Only once Pious has swapped settings at all.
        if accounts.is_empty() || (profiles.current().is_none() && accounts.iter().all(Option::is_none)) {
            return;
        }
        let _ = tokio::task::spawn_blocking(move || {
            // Give Roblox a moment to finish writing its file.
            std::thread::sleep(Duration::from_millis(800));
            for account in accounts {
                let _ = profiles.capture(account);
            }
        })
        .await;
    }

    /// Brings Pious back when it hid itself for a game and no Roblox window
    /// is left.
    pub fn show_after_games(self: &Shared) {
        let show = {
            let mut s = self.read();
            let show = s.hidden_for_game && s.instances.is_empty();
            if show {
                s.hidden_for_game = false;
            }
            show
        };
        if show {
            self.show_main();
        }
    }

    /// With the adjustable see-through blur, sends the main window a small,
    /// fresh capture of what's behind it about 15 times a second; the window
    /// blurs it at the chosen strength.
    async fn backdrop_loop(self: Shared) {
        use tauri::Manager;
        let mut last: u64 = 0;
        loop {
            // ~10 frames a second is plenty for a blurred backdrop.
            tokio::time::sleep(Duration::from_millis(100)).await;
            let wanted = {
                let s = self.read();
                let look = &s.bootstrapper.preferences.appearance;
                // Never while a game is in front: screen captures 10 times a
                // second cost the game frames, for a window nobody's looking at.
                look.see_through && look.blur == crate::core::model::Blur::Adjustable && !s.in_game
            };
            if !wanted {
                tokio::time::sleep(Duration::from_millis(400)).await;
                continue;
            }
            let Some(window) = self.app().get_webview_window("main") else { continue };
            if !window.is_visible().unwrap_or(false) || window.is_minimized().unwrap_or(false) {
                continue;
            }
            let (Ok(position), Ok(size)) = (window.outer_position(), window.outer_size()) else { continue };
            let frame = tokio::task::spawn_blocking(move || {
                let capture = process::capture(position.x, position.y, size.width as i32, size.height as i32, 4)?;
                let image = image::RgbaImage::from_raw(capture.width, capture.height, capture.rgba)?;
                let mut jpeg = Vec::new();
                image::DynamicImage::ImageRgba8(image)
                    .to_rgb8()
                    .write_to(&mut std::io::Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
                    .ok()?;
                Some(jpeg)
            })
            .await
            .ok()
            .flatten();
            if let Some(jpeg) = frame {
                use base64::Engine as _;
                use std::hash::{Hash, Hasher};
                // Nothing moved behind the window: don't redraw it.
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                jpeg.hash(&mut hasher);
                let hash = hasher.finish();
                if hash == last {
                    continue;
                }
                last = hash;
                let url = format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(jpeg));
                let _ = self.app().emit_to("main", "backdrop-frame", url);
            }
        }
    }

    pub fn show_main(self: &Shared) {
        self.open_main(true);
    }
}

/// A cheap fingerprint of the plugins and themes folders: each folder in
/// them, and when it, its manifest and its stylesheet last changed. Reading
/// every plugin (walking its Roblox files) only happens when this changes.
fn folders_signature() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let modified = |p: &std::path::Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let mut roots = vec![store::plugins_dir(), store::themes_dir()];
    if cfg!(debug_assertions) {
        let repo = plugins::repo_root();
        roots.push(repo.join("plugins"));
        roots.push(repo.join("themes"));
    }
    for root in roots {
        root.hash(&mut hasher);
        modified(&root).hash(&mut hasher);
        let Ok(entries) = std::fs::read_dir(&root) else { continue };
        let mut folders: Vec<std::path::PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        folders.sort();
        for folder in folders {
            folder.hash(&mut hasher);
            modified(&folder).hash(&mut hasher);
            for file in ["plugin.json", "theme.json", "theme.css", "style.css"] {
                modified(&folder.join(file)).hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

/// The release notes of every version, newest first.
pub const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

/// Everything the window draws, as JSON.
pub fn snapshot(s: &State) -> Value {
    let library = &s.bootstrapper;
    // What Play does for each game, with the sidebar account filled in.
    let effective: HashMap<Uuid, _> = library.games.iter().map(|g| (g.id, library.effective_config(g))).collect();
    let usable: Vec<&str> = library.versions.iter().filter(|v| v.is_usable()).map(|v| v.hash.as_str()).collect();
    let titles: HashMap<&str, String> = library.versions.iter().map(|v| (v.hash.as_str(), v.title())).collect();
    let default_version = library
        .resolve_version(&crate::core::model::VersionChoice::Default)
        .map(|v| v.hash.as_str());
    let latest_installed = library.latest_installed().map(|v| v.hash.as_str());

    json!({
        "loaded": s.loaded,
        "load_error": s.load_error,
        "bootstrapper": library,
        "active_account": library.play_account().map(|a| a.id),
        "effective": effective,
        "usable_versions": usable,
        "version_titles": titles,
        "default_version": default_version,
        "latest_installed": latest_installed,
        "instances": s.instances,
        "installs": s.installs,
        "latest": s.latest,
        "latest_error": s.latest_error,
        "scanning": s.scanning,
        "recommendations": s.recommendations,
        "recs_loading": s.recs_loading,
        "recs_error": s.recs_error,
        "denied_flags": s.denied_flags,
        "bootstrappers": s.bootstrappers,
        "link_handler": s.link_handler,
        "builds": s.builds,
        "builds_loading": s.builds_loading,
        "builds_error": s.builds_error,
        "update": s.update,
        "current_version": updater::CURRENT,
        "images": s.images,
        "hopping": s.hopping,
        "data_dir": store::data_dir(),
        "versions_dir": library.preferences.versions_dir.clone().unwrap_or_else(store::default_versions_dir),
        "sign_in": s.sign_in.as_ref().map(|si| json!({ "verifying": si.verifying, "reauth": si.reauth })),
        "tweaked_versions": library
            .versions
            .iter()
            .filter(|v| crate::core::fscache::is_file(&crate::core::tweaks::applied_marker(&v.path)))
            .map(|v| v.hash.as_str())
            .collect::<Vec<_>>(),
        "fast_flags": crate::core::tweaks::fast_flags(&library.preferences.tweaks).ok(),
        "mods_dir": crate::core::tweaks::mods_dir(&store::data_dir()),
        "recorder": recorder::status_of(s),
        "friends": s.friends,
        "news": s.news,
        "font_presets": crate::core::fonts::presets(),
        "roblox_uninstallable": s.roblox_uninstallable,
        "browser_sessions": s.browser_sessions,
        "mcp": s.mcp,
        "plugins": plugins::with_enabled(&s.plugins, &library.preferences.plugins),
        "themes": s.themes,
        "themes_dir": store::themes_dir(),
        "plugins_dir": store::plugins_dir(),
        "install_dir": store::install_root(),
        "crashes": s.crashes,
        "plugin_status": s.plugin_runtime.status,
        "roblox_updating": s.roblox_updating,
        "installed": s.installed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes the snapshot for a library file, for previewing the
    /// interface outside the app:
    /// `PIOUS_SNAPSHOT_IN=library.json PIOUS_SNAPSHOT_OUT=snap.json cargo test dump_snapshot -- --ignored`
    #[test]
    #[ignore]
    fn dump_snapshot() {
        let input = std::env::var("PIOUS_SNAPSHOT_IN").unwrap();
        let output = std::env::var("PIOUS_SNAPSHOT_OUT").unwrap();
        let mut state = State::default();
        state.bootstrapper = serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
        state.bootstrapper.active_account = state.bootstrapper.default_account().map(|a| a.id);
        state.bootstrapper.preferences.migrate_defaults();
        state.loaded = true;
        state.images = store::cached_artwork();
        state.bootstrappers = crate::core::bootstrappers::detect();
        std::fs::write(output, snapshot(&state).to_string()).unwrap();
    }
}
