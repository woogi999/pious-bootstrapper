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
        if pipe.is_none() {
            pipe = Pipe::connect(APP_ID);
            if pipe.is_none() {
                continue;
            }
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

    fn set_activity(&mut self, activity: Option<&Activity>) -> std::io::Result<()> {
        let activity = activity.map(|a| {
            let mut assets = json!({});
            if let Some(image) = &a.image {
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
                "assets": assets,
                "status_display_type": a.display,
            });
            if !buttons.is_empty() {
                activity["buttons"] = json!(buttons);
            }
            activity
        });
        self.send(
            1,
            &json!({
                "cmd": "SET_ACTIVITY",
                "args": { "pid": std::process::id(), "activity": activity },
                "nonce": uuid::Uuid::new_v4().to_string(),
            }),
        )?;
        let (op, _) = self.receive()?;
        if op == 2 {
            return Err(std::io::Error::other("Discord closed the connection"));
        }
        Ok(())
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
