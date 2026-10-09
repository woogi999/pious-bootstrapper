//! Pious's own pop-ups for what happens on Roblox: a friend messaged you,
//! someone sent a friend request, a friend started playing, or Roblox has
//! new notifications. They stack in the bottom right corner of the screen
//! (over games too, if wanted) and open the right thing when clicked.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use serde::Serialize;
use serde_json::json;
use tauri::{Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};
use uuid::Uuid;

use super::{Service, Shared};
use crate::core::social::{self, FriendStatus};
use crate::core::{credentials, process};

pub const NOTIFY_WINDOW: &str = "notify";
const WIDTH: f64 = 380.0;

/// One pop-up.
#[derive(Debug, Clone, Serialize)]
pub struct Notice {
    pub id: String,
    /// "message", "friend_request", "friend_join" or "roblox".
    pub kind: &'static str,
    pub title: String,
    pub body: String,
    pub avatar: Option<String>,
    /// What clicking it does.
    pub action: Value,
    /// The game it's about, for the game template (banner, icon, name).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game: Option<crate::core::roblox::GameCard>,
}

impl Notice {
    pub fn new(id: String, kind: &'static str, title: String, body: String, avatar: Option<String>, action: Value) -> Self {
        Self { id, kind, title, body, avatar, action, game: None }
    }
}

type Value = serde_json::Value;

/// What was already seen, so only new things pop up.
#[derive(Default)]
struct Seen {
    account: Option<Uuid>,
    /// Conversation → when it last changed.
    conversations: HashMap<String, String>,
    requests: HashSet<u64>,
    in_game: HashSet<u64>,
    unread: u64,
    /// The first look only remembers what's there.
    primed: bool,
}

/// Pop-ups not yet picked up by their window.
static QUEUE: std::sync::Mutex<Vec<serde_json::Value>> = std::sync::Mutex::new(Vec::new());

/// Everything waiting to be shown, oldest first.
pub fn take_notices() -> Vec<serde_json::Value> {
    QUEUE.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default()
}

impl Service {
    pub(super) async fn notify_loop(self: Shared) {
        let mut seen = Seen::default();
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let (prefs, account, chat_focused) = {
                let s = self.read();
                let account = s.friends.account.or(s.bootstrapper.active_account);
                let chat_focused = self
                    .app()
                    .get_webview_window(super::windows::CHAT_WINDOW)
                    .and_then(|w| w.is_focused().ok())
                    .unwrap_or(false);
                (s.bootstrapper.preferences.notifications.clone(), account, chat_focused)
            };
            if !prefs.enabled {
                seen.primed = false;
                continue;
            }
            let Some(account) = account else { continue };
            if seen.account != Some(account) {
                seen = Seen { account: Some(account), ..Default::default() };
            }
            let Ok(token) = tokio::task::spawn_blocking(move || credentials::load_session(account)).await.unwrap_or(Err(String::new()))
            else {
                continue;
            };
            let my_id = self.read().bootstrapper.account(account).map(|a| a.user_id).unwrap_or(0);
            let friends = self.read().friends.list.clone();
            let friend = |id: u64| friends.iter().find(|f| f.id == id);
            let mut out = Vec::new();

            // Messages.
            if prefs.messages {
                if let Ok(list) = social::conversations(&token).await {
                    for c in &list {
                        let updated = c.updated.clone().unwrap_or_default();
                        let before = seen.conversations.insert(c.id.clone(), updated.clone());
                        let changed = before.as_deref() != Some(updated.as_str());
                        if !seen.primed || !c.unread || !changed || chat_focused {
                            continue;
                        }
                        let other = c.participants.iter().copied().find(|p| *p != my_id);
                        let who = other.and_then(friend);
                        let name = who.map(|f| f.display_name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| {
                            if c.name.is_empty() { "New message".into() } else { c.name.clone() }
                        });
                        out.push(Notice::new(
                            format!("message:{}:{updated}", c.id),
                            "message",
                            name,
                            c.preview.clone().unwrap_or_else(|| "Sent you a message".into()),
                            who.and_then(|f| f.avatar.clone()),
                            json!({ "chat": { "account": account, "user": other } }),
                        ));
                    }
                }
            }

            // Friend requests.
            if prefs.friend_requests {
                if let Ok(list) = social::friend_requests(&token).await {
                    let fresh: Vec<_> = list.iter().filter(|r| seen.requests.insert(r.id) && seen.primed).collect();
                    let pictures = social::headshots(&token, &fresh.iter().map(|r| r.id).collect::<Vec<_>>()).await;
                    for r in fresh {
                        out.push(Notice::new(
                            format!("request:{}", r.id),
                            "friend_request",
                            r.display_name.clone(),
                            format!("@{} wants to be friends", r.username),
                            pictures.get(&r.id).cloned(),
                            json!({ "url": "https://www.roblox.com/users/friends#!/friend-requests" }),
                        ));
                    }
                }
            }

            // Friends starting a game (from the list Pious keeps fresh).
            let playing: HashSet<u64> = friends.iter().filter(|f| f.status == FriendStatus::InGame).map(|f| f.id).collect();
            if prefs.friend_joins && seen.primed {
                for f in friends.iter().filter(|f| playing.contains(&f.id) && !seen.in_game.contains(&f.id)) {
                    let mut notice = Notice::new(
                        format!("join:{}:{}", f.id, chrono::Utc::now().timestamp()),
                        "friend_join",
                        f.display_name.clone(),
                        match &f.location {
                            Some(game) if !game.is_empty() => format!("Started playing {game}"),
                            _ => "Started playing".into(),
                        },
                        f.avatar.clone(),
                        json!({ "join": { "place": f.place_id, "job": f.job, "name": f.location, "account": account } }),
                    );
                    // The game's banner and icon, when Roblox shares which.
                    if let Some(place) = f.place_id {
                        notice.game = crate::core::roblox::game_card(place).await.ok();
                    }
                    out.push(notice);
                }
            }
            seen.in_game = playing;

            // Roblox's own notifications.
            if prefs.roblox {
                if let Ok(unread) = social::unread_notifications(&token).await {
                    if seen.primed && unread > seen.unread {
                        let new = unread - seen.unread;
                        out.push(Notice::new(
                            format!("roblox:{unread}"),
                            "roblox",
                            "Roblox".into(),
                            if new == 1 { "You have a new notification".into() } else { format!("You have {new} new notifications") },
                            None,
                            json!({ "url": "https://www.roblox.com/notifications" }),
                        ));
                    }
                    seen.unread = unread;
                }
            }

            seen.primed = true;
            if out.is_empty() {
                continue;
            }
            // Not over a game unless wanted.
            if !prefs.in_game && self.read().foreground_roblox.is_some() {
                continue;
            }
            self.notify(out);
        }
    }

    /// Shows pop-ups (making their window if needed). They wait in a queue
    /// the window empties with [`take_notices`]: a window that's just been
    /// made isn't listening yet, and would miss them.
    ///
    /// The window is made from a background task, never from the window
    /// thread itself: building a window there (or from a command that runs
    /// there, like "Try one" did) can hang on Windows, which left pop-ups,
    /// and the app, stuck. A failure is said, not swallowed.
    pub fn notify(self: &Shared, notices: Vec<Notice>) {
        if notices.is_empty() {
            return;
        }
        let (seconds, sound, style) = {
            let n = &self.read().bootstrapper.preferences.notifications;
            (n.seconds.clamp(2, 60), n.sound, n.style.clone())
        };
        super::taskbar::attention(self, notices.len() as u32);
        if let Ok(mut queue) = QUEUE.lock() {
            // A window that never came up mustn't grow the queue forever.
            let excess = queue.len().saturating_sub(20);
            queue.drain(..excess);
            queue.push(json!({ "notices": notices, "seconds": seconds, "sound": sound, "style": style }));
        }
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            let app = me.app().clone();
            let window = match app.get_webview_window(NOTIFY_WINDOW) {
                Some(window) => window,
                None => {
                    let built = WebviewWindowBuilder::new(&app, NOTIFY_WINDOW, WebviewUrl::App("index.html".into()))
                        .title("Pious notifications")
                        .decorations(false)
                        .transparent(true)
                        .shadow(false)
                        .always_on_top(true)
                        .skip_taskbar(true)
                        .resizable(false)
                        .focused(false)
                        .focusable(false)
                        .inner_size(WIDTH, 10.0)
                        .visible(false)
                        .build();
                    match built {
                        Ok(window) => window,
                        Err(error) => {
                            // Say so; the next pop-up tries again.
                            if let Ok(mut queue) = QUEUE.lock() {
                                queue.clear();
                            }
                            me.toast(super::Tone::Negative, format!("Couldn't show the pop-up ({error})."));
                            return;
                        }
                    }
                }
            };
            let _ = window.set_always_on_top(true);
            // The window takes the queue when it starts, and again on this.
            let _ = app.emit_to(NOTIFY_WINDOW, "notify", ());
        });
    }

    /// The pop-ups measured themselves: fit the window to them, in the
    /// bottom right corner of the screen the pointer is on. No pop-ups:
    /// the window hides.
    pub fn notify_resize(&self, height: f64) {
        let Some(window) = self.app().get_webview_window(NOTIFY_WINDOW) else { return };
        if height < 1.0 {
            let _ = window.hide();
            return;
        }
        let _ = window.set_size(LogicalSize::new(WIDTH, height));
        let monitor = self
            .app()
            .cursor_position()
            .ok()
            .and_then(|p| self.app().monitor_from_point(p.x, p.y).ok().flatten())
            .or_else(|| window.primary_monitor().ok().flatten());
        if let Some(m) = monitor {
            let area = m.work_area();
            let scale = m.scale_factor();
            let x = area.position.x + area.size.width as i32 - ((WIDTH + 12.0) * scale) as i32;
            let y = area.position.y + area.size.height as i32 - ((height + 12.0) * scale) as i32;
            let _ = window.set_position(PhysicalPosition::new(x, y));
        }
        // Never in recordings: they're private.
        if let Ok(hwnd) = window.hwnd() {
            process::exclude_from_capture(hwnd.0 as isize, true);
        }
        if !window.is_visible().unwrap_or(false) {
            let _ = window.show();
        }
    }
}
