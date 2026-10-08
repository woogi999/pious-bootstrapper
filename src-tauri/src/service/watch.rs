//! Watching running Roblox windows: their logs (to know their server, and
//! to rejoin after a disconnect) and anti-AFK nudges.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use uuid::Uuid;

use super::{InstanceStatus, Service, Shared, Tone};
use crate::core::clientlog::{self, LEFT_ON_PURPOSE, LogEvent, LogTail};
use super::LaunchOutcome;
use crate::core::model::AfkAction;
use crate::core::process::{self, Nudge};

/// How long after a disconnect to wait for a teleport's new join before
/// treating it as a real disconnect.
const TELEPORT_GRACE: Duration = Duration::from_secs(8);
/// Anti-AFK waits until you've stopped using the keyboard and mouse for
/// this long, so a key press never lands in the middle of your typing.
const USER_SAFE_SECONDS: u32 = 3;

/// What Pious knows about one running window beyond what the UI shows.
#[derive(Debug, Clone)]
pub struct Watch {
    pub since: SystemTime,
    pub log: Option<LogTail>,
    /// A disconnect not yet followed by a new join, and when it was seen.
    pub disconnected: Option<(u32, Instant)>,
    /// The player left on purpose (Leave, closing the window).
    pub left: bool,
    /// Pious is closing it (Close, server hop, rejoin).
    pub closing: bool,
    pub joined: bool,
}

impl Watch {
    pub fn new() -> Self {
        Self {
            since: SystemTime::now(),
            log: None,
            disconnected: None,
            left: false,
            closing: false,
            joined: false,
        }
    }
}

/// Short descriptions of Roblox's common disconnect reasons.
fn reason_text(reason: u32) -> String {
    match reason {
        267 => "You were kicked".into(),
        273 => "Disconnected: the account joined from somewhere else".into(),
        277 => "Lost connection to the server".into(),
        279 => "Couldn't connect to the server".into(),
        288 => "The server shut down".into(),
        _ => format!("Disconnected (error {reason})"),
    }
}

impl Service {
    pub(super) async fn watch_loop(self: Shared) {
        loop {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            let (joined, servers) = self.read_logs();
            if joined {
                // Discord's Join button needs the new server.
                self.sync_presence().await;
            }
            for (id, ip) in servers {
                self.locate_server(id, ip).await;
            }
            self.rejoin_disconnected().await;
        }
    }

    /// Looks up where a window's server is, says so (when wanted) and shows
    /// it in Instances.
    async fn locate_server(&self, id: Uuid, ip: String) {
        let (wanted, cached) = {
            let s = self.read();
            (s.bootstrapper.preferences.server_location, s.server_locations.get(&ip).cloned())
        };
        if !wanted {
            return;
        }
        let location = match cached {
            Some(location) => location,
            None => match crate::core::roblox::server_location(&ip).await {
                Ok(location) => location,
                Err(_) => return,
            },
        };
        let game = self.mutate(|s| {
            s.server_locations.insert(ip, location.clone());
            let instance = s.instances.iter_mut().find(|i| i.id == id)?;
            instance.location = Some(location.clone());
            let game = instance.game;
            s.bootstrapper.game(game).map(|g| g.name.clone())
        });
        if let Some(game) = game {
            self.toast(Tone::Neutral, format!("{game}: the server is in {location}"));
        }
    }

    /// Attaches each running window to its log and reads what's new.
    /// Returns whether any window joined a server, and the server addresses
    /// that came up.
    fn read_logs(&self) -> (bool, Vec<(Uuid, String)>) {
        let mut s = self.read();
        let running: Vec<(Uuid, bool)> = s
            .instances
            .iter()
            .map(|i| (i.id, i.status == InstanceStatus::Running))
            .collect();
        s.watches.retain(|id, _| running.iter().any(|(r, _)| r == id));

        let mut joins: Vec<(Uuid, String)> = Vec::new();
        let mut servers: Vec<(Uuid, String)> = Vec::new();
        // The flags Roblox refused. A launch's log replaces the last
        // launch's list; later reads of it add to it.
        let mut denied = std::collections::BTreeSet::new();
        let mut fresh_log = false;
        for (id, is_running) in running {
            if !is_running {
                continue;
            }
            let taken: Vec<PathBuf> = s.watches.values().filter_map(|w| w.log.as_ref().map(|l| l.path.clone())).collect();
            let watch = s.watches.entry(id).or_insert_with(Watch::new);
            if watch.log.is_none() {
                watch.log = clientlog::find_log(watch.since, &taken).map(LogTail::new);
                fresh_log |= watch.log.is_some();
            }
            let Some(log) = &mut watch.log else { continue };
            for event in log.read() {
                match event {
                    LogEvent::Joined { job, .. } => {
                        watch.joined = true;
                        watch.disconnected = None;
                        joins.push((id, job));
                    }
                    LogEvent::Server { ip } => servers.push((id, ip)),
                    LogEvent::Denied { flag } => {
                        denied.insert(flag);
                    }
                    LogEvent::Disconnected { reason } if reason == LEFT_ON_PURPOSE => watch.left = true,
                    LogEvent::Disconnected { reason } => {
                        if watch.disconnected.is_none() {
                            watch.disconnected = Some((reason, Instant::now()));
                        }
                    }
                }
            }
        }
        if fresh_log || !denied.is_empty() {
            let next = if fresh_log { denied } else { s.denied_flags.iter().cloned().chain(denied).collect() };
            if next != s.denied_flags {
                crate::core::cache::save("denied-flags", &next);
                s.denied_flags = next;
            }
        }
        let changed = !joins.is_empty();
        for (id, job) in joins {
            if let Some(instance) = s.instances.iter_mut().find(|i| i.id == id) {
                instance.job = Some(job);
            }
        }
        drop(s);
        if changed {
            self.emit();
        }
        (changed, servers)
    }

    /// Rejoins windows that were disconnected (and not by a teleport).
    async fn rejoin_disconnected(self: &Shared) {
        let due: Vec<(Uuid, u32)> = {
            let s = self.read();
            if !s.bootstrapper.preferences.auto_rejoin {
                return;
            }
            s.watches
                .iter()
                .filter(|(_, w)| !w.closing && !w.left)
                .filter_map(|(id, w)| {
                    let (reason, at) = w.disconnected?;
                    (at.elapsed() >= TELEPORT_GRACE).then_some((*id, reason))
                })
                .collect()
        };
        for (id, reason) in due {
            self.rejoin(id, &reason_text(reason)).await;
        }
    }

    /// Closes a window (if it's still open) and launches the same game
    /// again with the same account and server.
    pub(super) async fn rejoin(self: &Shared, id: Uuid, why: &str) {
        let target = self.mutate(|s| {
            // Whoever marks the window closing first does the rejoin (the
            // log watcher and the exit check can both get here).
            let watch = s.watches.get_mut(&id)?;
            if std::mem::replace(&mut watch.closing, true) {
                return None;
            }
            let instance = s.instances.iter().find(|i| i.id == id)?.clone();
            let plan = instance.plan.clone()?;
            let name = s.bootstrapper.game(instance.game).map(|g| g.name.clone()).unwrap_or_default();
            Some((instance.pid, plan, name))
        });
        let Some((pid, mut plan, name)) = target else { return };
        if let Some(pid) = pid {
            let _ = tokio::task::spawn_blocking(move || process::terminate(pid)).await;
        }
        self.mutate(|s| {
            s.instances.retain(|i| i.id != id);
            s.watches.remove(&id);
        });
        self.toast(Tone::Active, format!("{why}. Rejoining {name}…"));
        // A kicked or shut-down server usually can't be rejoined: find a new
        // one (private servers stay private).
        plan.job = None;
        plan.force = true;
        plan.remember = false;
        plan.pin_account = false;
        if let LaunchOutcome::SignIn { .. } | LaunchOutcome::AddAccount = self.launch(plan) {
            self.toast(Tone::Caution, format!("Couldn't rejoin {name}: the account needs to sign in again."));
        }
    }

    /// Called when a window's process ended. If it was showing a
    /// disconnect when it closed (the player closed the error, or Roblox
    /// gave up), rejoin. Closing a game normally never rejoins: without a
    /// logged disconnect, there's no telling a crash from the close button.
    pub(super) async fn on_exited(self: &Shared, id: Uuid) -> bool {
        let reason = {
            let s = self.read();
            if !s.bootstrapper.preferences.auto_rejoin {
                return false;
            }
            s.watches
                .get(&id)
                .filter(|w| !w.left && !w.closing)
                .and_then(|w| w.disconnected)
                .map(|(reason, _)| reason)
        };
        match reason {
            Some(reason) => {
                self.rejoin(id, &reason_text(reason)).await;
                true
            }
            None => false,
        }
    }

    // ── Anti-AFK ─────────────────────────────────────────────────────────

    /// Keeps every Roblox window (Pious's or not) from being kicked for
    /// idling, the way AntiAFK-RBX does it: each window gets a key press on
    /// its own schedule, sent only once you've stopped typing or moving the
    /// mouse for a few seconds, then focus goes straight back to what you
    /// were using. A window you're playing in resets its own timer.
    pub(super) async fn anti_afk_loop(self: Shared) {
        let mut last: HashMap<u32, Instant> = HashMap::new();
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let (enabled, every, nudge) = {
                let s = self.read();
                let afk = &s.bootstrapper.preferences.anti_afk;
                let nudge = match afk.action {
                    AfkAction::Jump => Nudge::Jump,
                    AfkAction::Walk => Nudge::Walk,
                    AfkAction::Zoom => Nudge::Zoom,
                };
                (afk.enabled, Duration::from_secs(afk.minutes.clamp(1, 18) as u64 * 60), nudge)
            };
            if !enabled {
                last.clear();
                continue;
            }

            let pids = process::roblox_pids();
            last.retain(|pid, _| pids.contains(pid));
            let idle = process::idle_seconds();
            let foreground = process::window_pid(process::foreground_window());

            let mut due = Vec::new();
            for pid in pids {
                let since = last.entry(pid).or_insert_with(Instant::now);
                // You're playing in this window: that counts as activity.
                if pid == foreground && idle < USER_SAFE_SECONDS {
                    *since = Instant::now();
                } else if since.elapsed() >= every {
                    due.push(pid);
                }
            }
            // Never cut into typing or mouse movement.
            if due.is_empty() || idle < USER_SAFE_SECONDS {
                continue;
            }
            let done = tokio::task::spawn_blocking(move || {
                due.into_iter().filter(|&pid| process::nudge(pid, nudge)).collect::<Vec<_>>()
            })
            .await
            .unwrap_or_default();
            for pid in done {
                last.insert(pid, Instant::now());
            }
        }
    }

    // ── Window title and cache cleaning ──────────────────────────────────

    /// Renames Roblox windows to the chosen title and, once a day, clears
    /// old Roblox logs and cache.
    pub(super) async fn housekeeping_loop(self: Shared) {
        let mut cleaned: Option<Instant> = None;
        // The Roblox icon picture, by what it was made from.
        let mut icon: Option<(String, std::sync::Arc<image::RgbaImage>)> = None;
        // A picture that couldn't be made (offline, a missing file): tried
        // again after a minute, not every few seconds.
        let mut failed: Option<(String, Instant)> = None;
        // Whether Roblox windows have Pious's icon, to undo when it's turned off.
        let mut applied = false;
        loop {
            tokio::time::sleep(Duration::from_secs(3)).await;
            // The Roblox window's icon.
            let wanted = {
                let s = self.read();
                let t = &s.bootstrapper.preferences.tweaks;
                t.player_icon.clone().filter(|_| t.enabled).map(|choice| (choice, t.player_icon_file.clone()))
            };
            if let Some((choice, file)) = wanted {
                let key = format!("{choice}:{}", file.as_ref().map(|f| f.display().to_string()).unwrap_or_default());
                let waiting = failed.as_ref().is_some_and(|(k, at)| *k == key && at.elapsed() < Duration::from_secs(60));
                if icon.as_ref().is_none_or(|(k, _)| *k != key) && !waiting {
                    let cache = crate::core::store::data_dir().join("cache").join("mods");
                    icon = match crate::core::playericon::load(&choice, file.as_deref(), &cache).await {
                        Ok(image) => {
                            failed = None;
                            Some((key, std::sync::Arc::new(image)))
                        }
                        Err(_) => {
                            failed = Some((key, Instant::now()));
                            None
                        }
                    };
                }
                if let Some((key, image)) = icon.clone() {
                    let pids: Vec<u32> = self.read().instances.iter().filter_map(|i| i.pid).collect();
                    let _ = tokio::task::spawn_blocking(move || {
                        for window in pids.into_iter().filter_map(process::window_of) {
                            crate::core::playericon::set(window, &key, &image);
                        }
                    })
                    .await;
                    applied = true;
                }
            } else if std::mem::take(&mut applied) {
                icon = None;
                let pids: Vec<u32> = self.read().instances.iter().filter_map(|i| i.pid).collect();
                let _ = tokio::task::spawn_blocking(move || {
                    for window in pids.into_iter().filter_map(process::window_of) {
                        crate::core::playericon::reset(window);
                    }
                })
                .await;
            }
            let (title, enabled, days, priority) = {
                let s = self.read();
                let t = &s.bootstrapper.preferences.tweaks;
                (t.window_title.trim().to_owned(), t.enabled, t.cleaner_days, t.priority.clone().filter(|_| t.enabled))
            };
            // Roblox's CPU priority (checked each time: Roblox can reset it).
            if let Some(priority) = priority {
                let pids: Vec<u32> = self.read().instances.iter().filter_map(|i| i.pid).collect();
                for pid in pids {
                    crate::core::compat::set_priority(pid, Some(&priority));
                }
            }
            if enabled && !title.is_empty() {
                let pids: Vec<u32> = self.read().instances.iter().filter_map(|i| i.pid).collect();
                // Waited for, so a slow window never piles up threads.
                let _ = tokio::task::spawn_blocking(move || {
                    for pid in pids {
                        process::set_title(pid, &title);
                    }
                })
                .await;
            }
            if let (true, Some(days)) = (enabled, days) {
                if cleaned.is_none_or(|at| at.elapsed() >= Duration::from_secs(24 * 3600)) {
                    cleaned = Some(Instant::now());
                    let _ = tokio::task::spawn_blocking(move || crate::core::cleaner::clean(days)).await;
                }
            }
        }
    }
}
