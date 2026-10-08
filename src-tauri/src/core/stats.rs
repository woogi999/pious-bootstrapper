//! Every statistic Pious can keep, as an append-only log
//! (`stats.jsonl` in the data folder): one JSON object per line with a
//! timestamp and what happened. Nothing is ever sent anywhere; it's kept
//! for summaries like a yearly recap.

use std::io::Write;
use std::sync::Mutex;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub fn file() -> std::path::PathBuf {
    crate::core::store::data_dir().join("stats.jsonl")
}

static LOCK: Mutex<()> = Mutex::new(());

/// Records that `event` happened, with any details.
pub fn record(event: &str, details: Value) {
    let mut line = json!({ "at": Utc::now().to_rfc3339(), "event": event });
    if let (Some(line), Value::Object(details)) = (line.as_object_mut(), details) {
        line.extend(details);
    }
    let text = format!("{line}\n");
    std::thread::spawn(move || {
        let _guard = LOCK.lock();
        let path = file();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = f.write_all(text.as_bytes());
        }
    });
}

/// Totals for the About page.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Summary {
    pub play_seconds: u64,
    pub sessions: u64,
    pub launches: u64,
    pub games: u64,
    pub clips: u64,
    pub recordings: u64,
    pub messages: u64,
    pub macro_runs: u64,
    pub clicks: u64,
    pub server_hops: u64,
    pub app_opens: u64,
    pub since: Option<String>,
    /// Most played games: (name, seconds).
    pub top_games: Vec<(String, u64)>,
}

pub fn summary() -> Summary {
    let Ok(text) = std::fs::read_to_string(file()) else { return Summary::default() };
    let mut s = Summary::default();
    let mut games: std::collections::HashMap<String, u64> = Default::default();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        if s.since.is_none() {
            s.since = v["at"].as_str().map(str::to_owned);
        }
        match v["event"].as_str().unwrap_or_default() {
            "session" => {
                let secs = v["seconds"].as_u64().unwrap_or(0);
                s.play_seconds += secs;
                s.sessions += 1;
                *games.entry(v["game"].as_str().unwrap_or("Roblox").to_owned()).or_default() += secs;
            }
            "launch" => s.launches += 1,
            "clip" => s.clips += 1,
            "recording" => s.recordings += 1,
            "message" => s.messages += 1,
            "macro" => s.macro_runs += 1,
            "autoclick" => s.clicks += v["clicks"].as_u64().unwrap_or(0),
            "server_hop" => s.server_hops += 1,
            "app_open" => s.app_opens += 1,
            _ => {}
        }
    }
    s.games = games.len() as u64;
    let mut top: Vec<(String, u64)> = games.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1));
    top.truncate(5);
    s.top_games = top;
    s
}
