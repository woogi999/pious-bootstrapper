//! Discord Rich Presence over Discord's local IPC pipe: shows the game
//! you're playing in Pious on your Discord profile.
//!
//! A background thread owns the connection. It connects when there's
//! something to show, reconnects if Discord restarts, and clears the
//! status when nothing is running.

use std::io::{Read, Write};
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use serde_json::{Value, json};

/// Pious's Discord application (discord.com/developers → Pious → General
/// Information → Application ID). Its name, "Pious", is what Discord shows
/// when the status isn't set to show the game's name instead.
pub const APP_ID: &str = "1557737742148837490";

/// A public Discord application named "Roblox", with Roblox's logo as its
/// "roblox" picture (Bloxstrap's, which other bootstrappers use too).
pub const ROBLOX_APP_ID: &str = "1005469189907173486";

/// The application for a [`DiscordApp`] choice, and the name of its own
/// logo picture (shown when there's no game picture).
pub fn application(app: crate::core::model::DiscordApp) -> (&'static str, Option<&'static str>) {
    match app {
        crate::core::model::DiscordApp::Roblox => (ROBLOX_APP_ID, Some("roblox")),
        crate::core::model::DiscordApp::Pious => (APP_ID, None),
    }
}

/// Pious's Windows app ID (also its bundle identifier).
pub const APP_USER_MODEL_ID: &str = "com.pious.bootstrapper";

/// Registers Pious with Windows and Discord before anything talks to
/// Discord, like Discord's own `Discord_Register`:
/// - an explicit app ID for this process, so Windows (and Discord, which
///   asks Windows) knows every Pious window as one app;
/// - `discord-<application ID>` as a link that opens this exe, which is how
///   Discord finds the program behind an application (for "Join" and
///   "Ask to join" and for showing it as a game on the profile).
///
/// The Roblox application belongs to another project too, so its link is
/// only registered when nothing else (a bootstrapper) has claimed it.
pub fn register() {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
        let id: Vec<u16> = APP_USER_MODEL_ID.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let _ = SetCurrentProcessExplicitAppUserModelID(windows::core::PCWSTR(id.as_ptr()));
        }
        let Ok(exe) = std::env::current_exe() else { return };
        // A development build doesn't take the link from an installed Pious.
        register_link(APP_ID, &exe, !cfg!(debug_assertions));
        register_link(ROBLOX_APP_ID, &exe, false);
    }
}

/// Writes `HKCU\Software\Classes\discord-<app>` pointing at `exe`.
/// Without `replace`, a link something else already registered is kept.
#[cfg(windows)]
fn register_link(app: &str, exe: &std::path::Path, replace: bool) {
    use crate::core::compat::registry;
    let key = format!("Software\\Classes\\discord-{app}");
    let command = format!("\"{}\"", exe.display());
    let current = registry::get(&format!("{key}\\shell\\open\\command"), "");
    if current.as_deref() == Some(command.as_str()) {
        return;
    }
    // Someone else's (and still there): leave it.
    if !replace {
        if let Some(other) = &current {
            let other_exe = other.trim().trim_start_matches('"').split('"').next().unwrap_or("");
            if !other_exe.is_empty() && std::path::Path::new(other_exe).is_file() {
                return;
            }
        }
    }
    registry::set(&key, "", &format!("URL:Run game {app} protocol"));
    registry::set(&key, "URL Protocol", "");
    registry::set(&format!("{key}\\DefaultIcon"), "", &exe.display().to_string());
    registry::set(&format!("{key}\\shell\\open\\command"), "", &command);
}

/// What to show on the profile.
#[derive(Debug, Clone, PartialEq)]
pub struct Activity {
    /// The game's name.
    pub details: String,
    /// A second line, e.g. "Private server" or "Public server".
    pub state: String,
    /// When play started, as a Unix timestamp in seconds.
    pub started: i64,
    /// A picture of the game, by URL.
    pub image: Option<String>,
    pub place_id: u64,
    /// What Discord shows as the activity name: 0 = the application's name,
    /// 2 = the details line (the game's name).
    pub display: u8,
    /// A link that joins the same server (shown as a "Join game" button to
    /// everyone else; Discord never shows your own buttons to you).
    pub join_url: Option<String>,
    /// Another button: its label and link.
    pub button: Option<(String, String)>,
    /// The small picture in the corner and its tooltip (the account
    /// playing).
    pub small: Option<(String, String)>,
    /// The Discord application it's shown as (see [`application`]).
    pub app_id: &'static str,
    /// The application's own picture, for when there's no game picture.
    pub fallback_image: Option<&'static str>,
}

/// The link that joins one public server of a game. Roblox (or Pious, when
/// it opens Roblox links) starts it straight into that server.
pub fn join_link(place_id: u64, job: &str) -> String {
    format!("roblox://experiences/start?placeId={place_id}&gameInstanceId={job}")
}

enum Command {
    Set(Option<Activity>),
}

static SENDER: OnceLock<Sender<Command>> = OnceLock::new();

fn sender() -> &'static Sender<Command> {
    SENDER.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("discord-presence".into())
            .spawn(move || run(rx))
            .expect("Discord presence thread");
        tx
    })
}

/// Shows `activity` on Discord, or clears it with `None`. Cheap to call
/// repeatedly; only changes are sent.
pub fn set(activity: Option<Activity>) {
    // Nothing was ever shown: no need to start talking to Discord.
    if activity.is_none() && SENDER.get().is_none() {
        return;
    }
    let _ = sender().send(Command::Set(activity));
}

fn run(commands: Receiver<Command>) {
    let mut pipe: Option<Pipe> = None;
    // The application the open connection belongs to.
    let mut connected: &'static str = "";
    let mut shown: Option<Activity> = None;
    let mut wanted: Option<Activity> = None;

    loop {
        // Wait for a change, but wake up now and then to reconnect.
        match commands.recv_timeout(Duration::from_secs(15)) {
            Ok(Command::Set(activity)) => wanted = activity,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        // Coalesce bursts of updates.
        while let Ok(Command::Set(activity)) = commands.try_recv() {
            wanted = activity;
        }

        if wanted == shown && (pipe.is_some() || wanted.is_none()) {
            continue;
        }
        if wanted.is_none() && pipe.is_none() {
            shown = None;
            continue;
        }
        // Another application chosen: Discord ties a connection to one, so
        // the old status goes (with the old connection) and a new one opens.
        if let Some(app) = wanted.as_ref().map(|a| a.app_id) {
            if pipe.is_some() && app != connected {
                if let Some(old) = pipe.as_mut() {
                    let _ = old.set_activity(None);
                }
                pipe = None;
            }
        }
        if pipe.is_none() {
            let app = wanted.as_ref().map_or(connected, |a| a.app_id);
            pipe = Pipe::connect(if app.is_empty() { APP_ID } else { app });
            if pipe.is_none() {
                continue;
            }
            connected = if app.is_empty() { APP_ID } else { app };
        }
        let ok = pipe.as_mut().is_some_and(|p| p.set_activity(wanted.as_ref()).is_ok());
        if ok {
            shown = wanted.clone();
            // Nothing to show: let go of Discord entirely.
            if wanted.is_none() {
                pipe = None;
            }
        } else {
            pipe = None;
            shown = None;
        }
    }
}

struct Pipe {
    file: std::fs::File,
}

impl Pipe {
    fn connect(app_id: &str) -> Option<Self> {
        (0..10).find_map(|i| {
            let path = pipe_path(i);
            let file = std::fs::OpenOptions::new().read(true).write(true).open(path).ok()?;
            let mut pipe = Pipe { file };
            pipe.send(0, &json!({ "v": 1, "client_id": app_id })).ok()?;
            let (_, ready) = pipe.receive().ok()?;
            (ready["evt"] == "READY").then_some(pipe)
        })
    }

    /// Sets (or clears) the activity and waits for Discord's answer to it.
    /// Discord answers a field it doesn't know (`status_display_type` on
    /// older clients) with an error, not by ignoring it, so then it's sent
    /// again without it (the game's name still shows on the first line).
    fn set_activity(&mut self, activity: Option<&Activity>) -> std::io::Result<()> {
        match self.send_activity(activity, true) {
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData && activity.is_some() => self.send_activity(activity, false),
            other => other,
        }
    }

    fn send_activity(&mut self, activity: Option<&Activity>, display_type: bool) -> std::io::Result<()> {
        let activity = activity.map(|a| {
            let mut assets = json!({});
            if let Some(image) = a.image.as_deref().or(a.fallback_image) {
                assets["large_image"] = json!(image);
                assets["large_text"] = json!(a.details);
            }
            if let Some((image, text)) = &a.small {
                assets["small_image"] = json!(image);
                assets["small_text"] = json!(truncate(text));
            }
            // Discord allows two buttons.
            let mut buttons = Vec::new();
            if let Some(url) = &a.join_url {
                buttons.push(json!({ "label": "Join game", "url": url }));
            }
            if a.place_id != 0 {
                buttons.push(json!({ "label": "See game page", "url": format!("https://www.roblox.com/games/{}", a.place_id) }));
            }
            if let Some((label, url)) = &a.button {
                buttons.push(json!({ "label": label, "url": url }));
            }
            buttons.truncate(2);
            let mut activity = json!({
                "details": truncate(&a.details),
                "state": truncate(&a.state),
                "timestamps": { "start": a.started },
            });
            if display_type {
                activity["status_display_type"] = json!(a.display);
            }
            if assets.as_object().is_some_and(|o| !o.is_empty()) {
                activity["assets"] = assets;
            }
            if !buttons.is_empty() {
                activity["buttons"] = json!(buttons);
            }
            activity
        });
        let nonce = uuid::Uuid::new_v4().to_string();
        self.send(
            1,
            &json!({
                "cmd": "SET_ACTIVITY",
                "args": { "pid": std::process::id(), "activity": activity },
                "nonce": nonce,
            }),
        )?;
        // Other frames (events) can come first: wait for the answer to this.
        for _ in 0..8 {
            let (op, reply) = self.receive()?;
            if op == 2 {
                return Err(std::io::Error::other("Discord closed the connection"));
            }
            if reply["nonce"].as_str() != Some(nonce.as_str()) {
                continue;
            }
            if reply["evt"] == "ERROR" {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, reply["data"]["message"].as_str().unwrap_or("Discord refused the status").to_owned()));
            }
            return Ok(());
        }
        Err(std::io::Error::other("Discord didn't answer"))
    }

    fn send(&mut self, op: u32, payload: &Value) -> std::io::Result<()> {
        let body = serde_json::to_vec(payload)?;
        let mut frame = Vec::with_capacity(8 + body.len());
        frame.extend_from_slice(&op.to_le_bytes());
        frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
        frame.extend_from_slice(&body);
        self.file.write_all(&frame)
    }

    fn receive(&mut self) -> std::io::Result<(u32, Value)> {
        let mut header = [0u8; 8];
        self.file.read_exact(&mut header)?;
        let op = u32::from_le_bytes(header[..4].try_into().unwrap_or_default());
        let len = u32::from_le_bytes(header[4..].try_into().unwrap_or_default()) as usize;
        let mut body = vec![0u8; len.min(1 << 20)];
        self.file.read_exact(&mut body)?;
        Ok((op, serde_json::from_slice(&body).unwrap_or(Value::Null)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_lines_are_padded_long_ones_cut() {
        assert_eq!(truncate("a").chars().count(), 3);
        assert_eq!(truncate(&"x".repeat(300)).chars().count(), 127);
        assert_eq!(truncate("Adopt Me!"), "Adopt Me!");
    }
}

/// Discord limits each line to 128 characters.
fn truncate(text: &str) -> String {
    let mut out: String = text.chars().take(127).collect();
    if out.chars().count() < 2 {
        // Discord rejects lines shorter than two characters.
        out.push_str("  ");
    }
    out
}

#[cfg(windows)]
fn pipe_path(index: u32) -> String {
    format!(r"\\.\pipe\discord-ipc-{index}")
}

#[cfg(not(windows))]
fn pipe_path(index: u32) -> String {
    let dir = std::env::var("XDG_RUNTIME_DIR")
        .or_else(|_| std::env::var("TMPDIR"))
        .unwrap_or_else(|_| "/tmp".into());
    format!("{dir}/discord-ipc-{index}")
}
