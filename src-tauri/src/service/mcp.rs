//! A Model Context Protocol server, so AI apps (Claude and others) can see
//! and use Pious: list games and friends, launch, join, record, run macros.
//!
//! It listens on 127.0.0.1 only, over MCP's HTTP transport (JSON-RPC posts
//! to `/mcp`), and every request needs the token in the data folder.
//! Apps that only speak stdio run `pious.exe --mcp`, which relays to the
//! running Pious (starting it in the tray if needed). Apps that can't send
//! headers (ChatGPT's connectors) may put the token in the address instead:
//! `/mcp?token=<token>`.
//!
//! Besides Pious itself, the tools reach into Roblox windows: they can take
//! pictures of them, listen to them, and press keys, type and click in
//! them in the background.

use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

use super::{LaunchOutcome, Service, Shared};
use crate::core::{stats, store};

const PROTOCOL: &str = "2025-06-18";

#[derive(Debug, Clone, Default, Serialize)]
pub struct McpState {
    /// The address apps connect to, while the server runs.
    pub url: Option<String>,
    pub error: Option<String>,
    #[serde(skip)]
    pub generation: u64,
}

/// The token apps send as `Authorization: Bearer <token>`.
pub fn token() -> String {
    let path = store::data_dir().join("mcp-token.txt");
    if let Ok(token) = std::fs::read_to_string(&path) {
        let token = token.trim().to_owned();
        if token.len() >= 32 {
            return token;
        }
    }
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let _ = std::fs::create_dir_all(store::data_dir());
    let _ = std::fs::write(&path, &token);
    token
}

impl Service {
    /// Starts, restarts or stops the server to match the settings.
    pub fn restart_mcp(self: &Shared) {
        let (enabled, port, generation) = self.mutate(|s| {
            s.mcp.generation += 1;
            s.mcp.url = None;
            s.mcp.error = None;
            (s.bootstrapper.preferences.mcp.enabled, s.bootstrapper.preferences.mcp.port, s.mcp.generation)
        });
        if !enabled {
            return;
        }
        let me = self.clone();
        tauri::async_runtime::spawn(async move { me.serve_mcp(port, generation).await });
    }

    async fn serve_mcp(self: Shared, port: u16, generation: u64) {
        // On a restart the previous server lets go of the port within a
        // second (it checks between accepts), so keep trying a little.
        let mut attempt = 0;
        let listener = loop {
            match TcpListener::bind(("127.0.0.1", port)).await {
                Ok(listener) => break listener,
                Err(_) if attempt < 8 => {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    if self.read().mcp.generation != generation {
                        return;
                    }
                }
                Err(error) => {
                    self.mutate(|s| s.mcp.error = Some(format!("Port {port} is in use ({error}). Pick another port.")));
                    return;
                }
            }
        };
        let secret = token();
        self.mutate(|s| s.mcp.url = Some(format!("http://127.0.0.1:{port}/mcp")));
        loop {
            let accepted = tokio::time::timeout(Duration::from_secs(1), listener.accept()).await;
            if self.read().mcp.generation != generation {
                return;
            }
            let Ok(Ok((stream, _))) = accepted else { continue };
            let me = self.clone();
            let secret = secret.clone();
            tauri::async_runtime::spawn(async move {
                let _ = me.mcp_connection(stream, &secret, generation).await;
            });
        }
    }

    async fn mcp_connection(self: Shared, stream: TcpStream, secret: &str, generation: u64) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream);
        loop {
            // The request line and headers.
            let mut line = String::new();
            if reader.read_line(&mut line).await? == 0 {
                return Ok(());
            }
            let mut parts = line.split_whitespace();
            let method = parts.next().unwrap_or_default().to_owned();
            let path = parts.next().unwrap_or_default().to_owned();
            let mut length = 0usize;
            let mut auth = String::new();
            let mut origin = String::new();
            let mut close = false;
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).await? == 0 {
                    return Ok(());
                }
                let header = header.trim_end();
                if header.is_empty() {
                    break;
                }
                if let Some((name, value)) = header.split_once(':') {
                    let value = value.trim();
                    match name.trim().to_ascii_lowercase().as_str() {
                        "content-length" => length = value.parse().unwrap_or(0),
                        "authorization" => auth = value.to_owned(),
                        "origin" => origin = value.to_owned(),
                        "connection" => close = value.eq_ignore_ascii_case("close"),
                        _ => {}
                    }
                }
            }
            if length > 4 * 1024 * 1024 {
                return respond(reader.get_mut(), 413, "", true).await;
            }
            let mut body = vec![0u8; length];
            reader.read_exact(&mut body).await?;
            // Turned off (or restarted) since this connection opened.
            if self.read().mcp.generation != generation {
                return Ok(());
            }

            // Web pages can't reach it (a page could point a name at
            // 127.0.0.1); only local apps with the token can.
            let local_origin = origin.is_empty()
                || origin.starts_with("http://127.0.0.1")
                || origin.starts_with("http://localhost");
            let query_token = path
                .split_once('?')
                .and_then(|(_, q)| q.split('&').find_map(|kv| kv.strip_prefix("token=")))
                .map(str::to_owned);
            let authorized = auth.strip_prefix("Bearer ").map(str::trim) == Some(secret) || query_token.as_deref() == Some(secret);
            let (status, reply) = if !local_origin {
                (403, String::new())
            } else if !authorized {
                (401, json!({ "error": "Pious needs its MCP token (Settings → Plugins)." }).to_string())
            } else if path.split('?').next() != Some("/mcp") {
                (404, String::new())
            } else if method == "POST" {
                match serde_json::from_slice::<Value>(&body) {
                    Ok(Value::Array(batch)) => {
                        let mut out = Vec::new();
                        for message in batch {
                            if let Some(reply) = self.mcp_message(message).await {
                                out.push(reply);
                            }
                        }
                        if out.is_empty() { (202, String::new()) } else { (200, Value::Array(out).to_string()) }
                    }
                    Ok(message) => match self.mcp_message(message).await {
                        Some(reply) => (200, reply.to_string()),
                        None => (202, String::new()),
                    },
                    Err(_) => (400, rpc_error(Value::Null, -32700, "Not JSON").to_string()),
                }
            } else if method == "DELETE" {
                (200, String::new())
            } else {
                // No server-sent events: everything is answered directly.
                (405, String::new())
            };
            respond(reader.get_mut(), status, &reply, close).await?;
            if close {
                return Ok(());
            }
        }
    }

    /// Answers one JSON-RPC message (`None` for notifications).
    async fn mcp_message(self: &Shared, message: Value) -> Option<Value> {
        let id = message.get("id").cloned()?;
        let method = message["method"].as_str().unwrap_or_default();
        let params = &message["params"];
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": params["protocolVersion"].as_str().unwrap_or(PROTOCOL),
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "pious", "title": "Pious", "version": crate::core::updater::CURRENT },
                "instructions": "Pious is a Roblox launcher. Use these tools to see the user's games, accounts, friends and running games, and (when allowed) to launch games, join friends, record clips and run macros. You can also look at a running Roblox window (screenshot, watch), hear it (listen), and play in it: walk, jump, press keys, type in chat and click (move_character, press_keys, type_text, click, run_input). Input goes to the window in the background, so the user's own mouse and keyboard stay free. Look before and after acting to see what happened.",
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => {
                let name = params["name"].as_str().unwrap_or_default().to_owned();
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                stats::record("mcp", json!({ "tool": name }));
                let outcome = self.mcp_tool(&name, &args).await;
                Ok(match outcome {
                    // Pictures and sound, already as MCP content.
                    Ok(value) if value.get(CONTENT).is_some() => json!({ "content": value[CONTENT].clone(), "isError": false }),
                    Ok(value) => json!({
                        "content": [{ "type": "text", "text": if value.is_string() { value.as_str().unwrap_or_default().to_owned() } else { serde_json::to_string_pretty(&value).unwrap_or_default() } }],
                        "isError": false,
                    }),
                    Err(error) => json!({ "content": [{ "type": "text", "text": error }], "isError": true }),
                })
            }
            _ => Err((-32601, format!("Unknown method {method}"))),
        };
        Some(match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, text)) => rpc_error(id, code, &text),
        })
    }

    async fn mcp_tool(self: &Shared, name: &str, args: &Value) -> Result<Value, String> {
        let read_only = matches!(name, "get_status" | "list_games" | "list_accounts" | "list_friends" | "list_macros" | "get_stats" | "list_instances");
        let senses = matches!(name, "screenshot" | "watch" | "listen");
        let prefs = self.read().bootstrapper.preferences.mcp.clone();
        if senses && !prefs.allow_senses {
            return Err("Pious doesn't let AI apps see or hear games. Turn on \"See and hear games\" in Pious's settings.".into());
        }
        if !read_only && !senses && !prefs.allow_actions {
            return Err("Pious only lets AI apps look, not act. Turn on \"Allow actions\" in Pious's settings.".into());
        }
        let text = |key: &str| args[key].as_str().map(str::trim).filter(|t| !t.is_empty()).map(str::to_owned);
        match name {
            "get_status" => {
                // Macros live in their plugin: ask it (nothing, if it's off).
                let macros = self.plugin_request("macros", "status", json!({})).await.ok();
                let s = self.read();
                let lib = &s.bootstrapper;
                Ok(json!({
                    "running": s.instances.iter().map(|i| json!({
                        "id": i.id,
                        "game": lib.game(i.game).map(|g| g.name.clone()),
                        "account": i.account.and_then(|a| lib.account(a)).map(|a| a.username.clone()),
                        "since": i.started,
                    })).collect::<Vec<_>>(),
                    "play_as": lib.play_account().map(|a| a.username.clone()),
                    "recording": s.capture.recording.is_some(),
                    "macros_running": macros.as_ref().and_then(|m| m["running"].as_array().map(Vec::len)).unwrap_or(0),
                    "autoclicker_on": macros.as_ref().and_then(|m| m["clicking"].as_bool()).unwrap_or(false),
                    "pious_version": crate::core::updater::CURRENT,
                }))
            }
            "list_games" => {
                let s = self.read();
                Ok(json!(s.bootstrapper.games.iter().filter(|g| g.in_library).map(|g| json!({
                    "id": g.id, "name": g.name, "place_id": g.place_id, "favorite": g.favorite,
                    "last_played": g.last_played, "play_count": g.play_count, "collection": g.collection,
                })).collect::<Vec<_>>()))
            }
            "list_accounts" => {
                let s = self.read();
                Ok(json!(s.bootstrapper.accounts.iter().map(|a| json!({
                    "id": a.id, "username": a.username, "display_name": a.display_name, "nickname": a.alias,
                    "needs_sign_in": a.needs_sign_in, "play_as": s.bootstrapper.active_account == Some(a.id),
                })).collect::<Vec<_>>()))
            }
            "list_friends" => {
                let s = self.read();
                Ok(json!(s.friends.list.iter().map(|f| json!({
                    "username": f.username, "display_name": f.display_name, "status": f.status,
                    "playing": f.location, "place_id": f.place_id,
                })).collect::<Vec<_>>()))
            }
            // Macros live in the Macros plugin: these ask it.
            "list_macros" => self.plugin_request("macros", "list", json!({})).await,
            "get_stats" => Ok(serde_json::to_value(stats::summary()).unwrap_or_default()),
            "launch_game" => {
                let wanted = text("game").ok_or("Say which game (its name, Pious ID or place ID).")?;
                let account = text("account").map(|a| self.find_account(&a)).transpose()?;
                let game = self.find_game(&wanted)?;
                if let Some(account) = account {
                    self.mutate(|s| s.bootstrapper.active_account = Some(account));
                }
                Ok(json!(describe(self.play(game, None, args["force"].as_bool().unwrap_or(false)))))
            }
            "join_friend" => {
                let wanted = text("friend").ok_or("Say which friend.")?.to_lowercase();
                let friend = self
                    .read()
                    .friends
                    .list
                    .iter()
                    .find(|f| f.username.to_lowercase() == wanted || f.display_name.to_lowercase() == wanted)
                    .cloned()
                    .ok_or("No friend by that name.")?;
                let place = friend.place_id.ok_or(format!("{} isn't in a game you can join.", friend.display_name))?;
                let account = self.read().friends.account;
                Ok(json!(describe(self.join_player(place, friend.job.clone(), account, friend.location.clone(), false))))
            }
            "join_server_link" => {
                let link = text("link").ok_or("Give the server link.")?;
                Ok(json!(describe(self.join_link(link, None, false).await)))
            }
            "server_hop" => {
                let game = text("game").map(|g| self.find_game(&g)).transpose()?;
                let instance = match game {
                    Some(_) => None,
                    None => Some(self.read().instances.first().map(|i| i.id).ok_or("No game is running; say which game.")?),
                };
                Ok(json!(describe(self.server_hop(game, instance, false).await)))
            }
            "close_game" => {
                match text("id") {
                    Some(id) => self.close_instance(id.parse().map_err(|_| "That isn't a running game's ID.")?).await,
                    None => self.close_all().await,
                }
                Ok(json!("Closed."))
            }
            "toggle_recording" => {
                self.toggle_recording();
                Ok(json!("Done."))
            }
            "save_clip" => {
                let seconds = args["seconds"].as_u64().map(|s| s as u32);
                self.save_clip(seconds, std::time::Instant::now());
                Ok(json!("Saving the clip."))
            }
            "run_macro" => {
                let wanted = text("name").ok_or("Say which macro.")?;
                self.plugin_request("macros", "run", json!({ "name": wanted })).await
            }
            "stop_macros" => {
                self.plugin_request("macros", "stop", json!({})).await?;
                Ok(json!("Stopped."))
            }
            "list_instances" => {
                let s = self.read();
                let lib = &s.bootstrapper;
                Ok(json!(s.instances.iter().map(|i| json!({
                    "id": i.id,
                    "game": lib.game(i.game).map(|g| g.name.clone()),
                    "account": i.account.and_then(|a| lib.account(a)).map(|a| a.username.clone()),
                    "server": i.job,
                    "in_front": i.pid.is_some() && i.pid == s.foreground_roblox,
                    "since": i.started,
                })).collect::<Vec<_>>()))
            }
            "screenshot" => {
                let (window, label) = self.mcp_window(args)?;
                let width = args["max_width"].as_u64().unwrap_or(1280).clamp(160, 2560) as u32;
                let png = tokio::task::spawn_blocking(move || picture_png(window, width)).await.map_err(|e| e.to_string())??;
                Ok(json!({ CONTENT: [
                    { "type": "text", "text": format!("{label}, right now.") },
                    { "type": "image", "data": png, "mimeType": "image/png" },
                ] }))
            }
            "watch" => {
                let (window, label) = self.mcp_window(args)?;
                let seconds = args["seconds"].as_f64().unwrap_or(3.0).clamp(0.5, 15.0);
                let frames = args["frames"].as_u64().unwrap_or(4).clamp(2, 8) as u32;
                let width = args["max_width"].as_u64().unwrap_or(800).clamp(160, 1600) as u32;
                let gap = seconds / (frames - 1) as f64;
                let mut content = vec![json!({ "type": "text", "text": format!("{label}: {frames} pictures over {seconds:.1} seconds, oldest first.") })];
                for i in 0..frames {
                    if i > 0 {
                        tokio::time::sleep(Duration::from_secs_f64(gap)).await;
                    }
                    let png = tokio::task::spawn_blocking(move || picture_png(window, width)).await.map_err(|e| e.to_string())??;
                    content.push(json!({ "type": "text", "text": format!("At {:.1}s:", i as f64 * gap) }));
                    content.push(json!({ "type": "image", "data": png, "mimeType": "image/png" }));
                }
                Ok(json!({ CONTENT: content }))
            }
            "listen" => {
                let (pid, label) = self.mcp_instance(args).map(|(i, label)| (i.pid, label))?;
                let pid = pid.ok_or("That game's window isn't open yet.")?;
                let seconds = args["seconds"].as_f64().unwrap_or(5.0).clamp(1.0, 15.0);
                let wav = tokio::task::spawn_blocking(move || crate::core::recorder::listen(pid, seconds)).await.map_err(|e| e.to_string())??;
                use base64::Engine;
                Ok(json!({ CONTENT: [
                    { "type": "text", "text": format!("{seconds:.0} seconds of {label}'s sound (only the game, mono WAV).") },
                    { "type": "audio", "data": base64::engine::general_purpose::STANDARD.encode(wav), "mimeType": "audio/wav" },
                ] }))
            }
            "focus_instance" => {
                let (instance, label) = self.mcp_instance(args)?;
                self.focus_instance(instance.id).await;
                Ok(json!(format!("{label} is in front.")))
            }
            "press_keys" | "type_text" | "click" | "move_character" | "run_input" => {
                use crate::core::automation::{Button, Press, Step};
                let (window, label) = self.mcp_window(args)?;
                let steps: Vec<Step> = match name {
                    "press_keys" => {
                        let hold = args["hold_ms"].as_u64().unwrap_or(60).min(30_000) as u32;
                        let times = args["times"].as_u64().unwrap_or(1).clamp(1, 100);
                        let keys: Vec<String> = match &args["keys"] {
                            Value::Array(list) => list.iter().filter_map(|k| k.as_str().map(str::to_owned)).collect(),
                            Value::String(k) => k.split(['+', ',']).map(|k| k.trim().to_owned()).filter(|k| !k.is_empty()).collect(),
                            _ => Vec::new(),
                        };
                        let codes: Vec<String> = keys.iter().map(|k| key_code(k).ok_or(format!("Pious doesn't know the key {k}."))).collect::<Result<_, _>>()?;
                        if codes.is_empty() {
                            return Err("Say which keys to press.".into());
                        }
                        // Held together (a combination), then let go.
                        let mut once: Vec<Step> = codes.iter().map(|k| Step::Key { key: k.clone(), press: Press::Down, hold_ms: 0 }).collect();
                        once.push(Step::Wait { ms: hold, random_ms: 0 });
                        once.extend(codes.iter().rev().map(|k| Step::Key { key: k.clone(), press: Press::Up, hold_ms: 0 }));
                        once.push(Step::Wait { ms: 40, random_ms: 0 });
                        (0..times).flat_map(|_| once.clone()).collect()
                    }
                    "type_text" => {
                        let message = text("text").ok_or("Say what to type.")?;
                        let chat = args["chat"].as_bool().unwrap_or(true);
                        let mut steps = Vec::new();
                        if chat {
                            // Roblox opens its chat box with /.
                            steps.push(Step::Key { key: "Slash".into(), press: Press::Tap, hold_ms: 30 });
                            steps.push(Step::Wait { ms: 250, random_ms: 0 });
                        }
                        steps.push(Step::Text { text: message, delay_ms: 15 });
                        if chat || args["enter"].as_bool().unwrap_or(false) {
                            steps.push(Step::Wait { ms: 100, random_ms: 0 });
                            steps.push(Step::Key { key: "Enter".into(), press: Press::Tap, hold_ms: 30 });
                        }
                        steps
                    }
                    "click" => {
                        let x = args["x"].as_f64().unwrap_or(0.5);
                        let y = args["y"].as_f64().unwrap_or(0.5);
                        let button = match args["button"].as_str().unwrap_or("left") {
                            "right" => Button::Right,
                            "middle" => Button::Middle,
                            _ => Button::Left,
                        };
                        let count = args["count"].as_u64().unwrap_or(1).clamp(1, 10) as u32;
                        let (sx, sy) = crate::core::automation::window_point(window, x, y);
                        vec![Step::Move { x: sx, y: sy, relative: false, duration_ms: 0 }, Step::Click { button, press: Press::Tap, count }]
                    }
                    "move_character" => {
                        let seconds = args["seconds"].as_f64().unwrap_or(1.0).clamp(0.05, 30.0);
                        let ms = (seconds * 1000.0) as u32;
                        let keys: Vec<&str> = match args["direction"].as_str().unwrap_or("forward") {
                            "back" | "backward" => vec!["KeyS"],
                            "left" => vec!["KeyA"],
                            "right" => vec!["KeyD"],
                            "forward_left" => vec!["KeyW", "KeyA"],
                            "forward_right" => vec!["KeyW", "KeyD"],
                            "jump" => vec!["Space"],
                            _ => vec!["KeyW"],
                        };
                        let mut steps: Vec<Step> = keys.iter().map(|k| Step::Key { key: (*k).into(), press: Press::Down, hold_ms: 0 }).collect();
                        if args["jump"].as_bool().unwrap_or(false) && !keys.contains(&"Space") {
                            steps.push(Step::Key { key: "Space".into(), press: Press::Tap, hold_ms: 80 });
                        }
                        steps.push(Step::Wait { ms: if keys == ["Space"] { 120 } else { ms }, random_ms: 0 });
                        steps.extend(keys.iter().map(|k| Step::Key { key: (*k).into(), press: Press::Up, hold_ms: 0 }));
                        steps
                    }
                    _ => serde_json::from_value(args["steps"].clone()).map_err(|e| format!("Those steps don't read ({e}). See the tool's description for their shape."))?,
                };
                let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let sink = crate::core::automation::Sink::Windows(vec![window]);
                let finished = tokio::task::spawn_blocking(move || crate::core::automation::play(&steps, 1.0, &stop, &NoHost, &sink))
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(json!(if finished { format!("Done in {label}. Take a screenshot to see what happened.") } else { "Stopped part-way.".to_owned() }))
            }
            "send_message" => {
                let wanted = text("friend").ok_or("Say which friend.")?.to_lowercase();
                let message = text("text").ok_or("Say what to send.")?;
                let (account, user) = {
                    let s = self.read();
                    let friend = s
                        .friends
                        .list
                        .iter()
                        .find(|f| f.username.to_lowercase() == wanted || f.display_name.to_lowercase() == wanted)
                        .ok_or("No friend by that name.")?;
                    (s.friends.account.ok_or("No account is selected for friends.")?, friend.id)
                };
                let conversation = self.open_chat(account, user).await?;
                self.send_chat(account, conversation, message).await?;
                Ok(json!("Sent."))
            }
            _ => Err(format!("Pious has no tool called {name}.")),
        }
    }

    fn find_game(self: &Shared, wanted: &str) -> Result<Uuid, String> {
        if let Ok(id) = wanted.parse::<Uuid>() {
            return Ok(id);
        }
        if let Ok(place) = wanted.parse::<u64>() {
            return Ok(self.game_for_place(place, None));
        }
        let s = self.read();
        let lower = wanted.to_lowercase();
        let games = &s.bootstrapper.games;
        games
            .iter()
            .find(|g| g.name.to_lowercase() == lower)
            .or_else(|| games.iter().find(|g| g.name.to_lowercase().contains(&lower)))
            .map(|g| g.id)
            .ok_or_else(|| format!("No game called {wanted} in Pious."))
    }

    /// The running game a tool means: `instance` as a running game's ID, its
    /// game's name or its account's name; else the one in front, else the
    /// newest.
    fn mcp_instance(&self, args: &Value) -> Result<(super::Instance, String), String> {
        let s = self.read();
        let lib = &s.bootstrapper;
        if s.instances.is_empty() {
            return Err("No Roblox game is running. Start one with launch_game first.".into());
        }
        let wanted = args["instance"].as_str().map(|w| w.trim().to_lowercase()).filter(|w| !w.is_empty());
        let found = match &wanted {
            Some(w) => s.instances.iter().find(|i| {
                i.id.to_string() == *w
                    || lib.game(i.game).is_some_and(|g| g.name.to_lowercase() == *w || g.name.to_lowercase().contains(w.as_str()))
                    || i.account.and_then(|a| lib.account(a)).is_some_and(|a| a.username.to_lowercase() == *w || a.display_name.to_lowercase() == *w)
            }),
            None => s
                .instances
                .iter()
                .find(|i| i.pid.is_some() && i.pid == s.foreground_roblox)
                .or_else(|| s.instances.iter().max_by_key(|i| i.started)),
        };
        let instance = found.cloned().ok_or("No running game matches that. list_instances shows them.")?;
        let label = format!(
            "{}{}",
            lib.game(instance.game).map(|g| g.name.clone()).unwrap_or_else(|| "Roblox".into()),
            instance.account.and_then(|a| lib.account(a)).map(|a| format!(" (as {})", a.username)).unwrap_or_default()
        );
        Ok((instance, label))
    }

    /// The window of the running game a tool means.
    fn mcp_window(&self, args: &Value) -> Result<(isize, String), String> {
        let (instance, label) = self.mcp_instance(args)?;
        let pid = instance.pid.ok_or("That game's window isn't open yet.")?;
        let window = crate::core::process::window_of(pid).ok_or("That game's window isn't open yet.")?;
        Ok((window, label))
    }

    fn find_account(&self, wanted: &str) -> Result<Uuid, String> {
        let s = self.read();
        let lower = wanted.to_lowercase();
        s.bootstrapper
            .accounts
            .iter()
            .find(|a| {
                a.username.to_lowercase() == lower
                    || a.display_name.to_lowercase() == lower
                    || a.alias.as_deref().is_some_and(|n| n.to_lowercase() == lower)
            })
            .map(|a| a.id)
            .ok_or_else(|| format!("No account called {wanted}."))
    }
}

/// Marks a tool result that is already MCP content (pictures, sound).
const CONTENT: &str = "__content";

/// Macros sent by AI apps never bring windows forward on their own.
struct NoHost;

impl crate::core::automation::Host for NoHost {
    fn focus_roblox(&self) {}
}

/// A picture of a window as base64 PNG, no wider than `width`.
fn picture_png(window: isize, width: u32) -> Result<String, String> {
    use base64::Engine;
    let (w, h, rgba) = crate::core::automation::window_picture(window).ok_or("Couldn't take a picture of that window (is it minimized?).")?;
    let image = image::RgbaImage::from_raw(w, h, rgba).ok_or("The picture came out wrong.")?;
    let image = if w > width {
        image::imageops::resize(&image, width, (h as f64 * width as f64 / w as f64).round().max(1.0) as u32, image::imageops::FilterType::Triangle)
    } else {
        image
    };
    let mut out = std::io::Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(out.into_inner()))
}

/// Key names the way people write them ("w", "space", "shift", "f5",
/// "KeyW") → Pious's.
fn key_code(name: &str) -> Option<String> {
    let lower = name.trim().to_lowercase();
    let named = match lower.as_str() {
        "space" | " " => "Space",
        "enter" | "return" => "Enter",
        "shift" => "ShiftLeft",
        "ctrl" | "control" => "ControlLeft",
        "alt" => "AltLeft",
        "tab" => "Tab",
        "esc" | "escape" => "Escape",
        "backspace" => "Backspace",
        "up" => "ArrowUp",
        "down" => "ArrowDown",
        "left" => "ArrowLeft",
        "right" => "ArrowRight",
        "/" | "slash" => "Slash",
        "." => "Period",
        "," => "Comma",
        "-" => "Minus",
        "=" => "Equal",
        _ => "",
    };
    if !named.is_empty() {
        return Some(named.into());
    }
    if crate::core::automation::vk_of(name.trim()).is_some() {
        return Some(name.trim().to_owned());
    }
    let mut chars = lower.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphabetic() => Some(format!("Key{}", c.to_ascii_uppercase())),
        (Some(c), None) if c.is_ascii_digit() => Some(format!("Digit{c}")),
        _ => lower.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()).filter(|n| (1..=24).contains(n)).map(|n| format!("F{n}")),
    }
}

fn describe(outcome: LaunchOutcome) -> Value {
    match outcome {
        LaunchOutcome::Started => json!("Starting the game."),
        LaunchOutcome::Stopped => json!("Nothing was started."),
        LaunchOutcome::Confirm { title, body, .. } => json!(format!("{title}: {body} (call again with force: true to go ahead)")),
        LaunchOutcome::SignIn { .. } => json!("That account must sign in again in Pious first."),
        LaunchOutcome::AddAccount => json!("Add a Roblox account in Pious first."),
        LaunchOutcome::Blocked(reason) => json!(reason),
    }
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

async fn respond(stream: &mut TcpStream, status: u16, body: &str, close: bool) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: {}\r\n\r\n",
        body.len(),
        if close { "close" } else { "keep-alive" }
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body.as_bytes()).await?;
    stream.flush().await
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str], read_only: bool) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required },
        "annotations": { "readOnlyHint": read_only },
    })
}

fn tools() -> Vec<Value> {
    let string = |d: &str| json!({ "type": "string", "description": d });
    let instance = string("Which running game: its ID from list_instances, its game's name or its account's name (default: the one in front)");
    vec![
        tool("get_status", "What's happening in Pious: running Roblox games, the play-as account, recording and macros.", json!({}), &[], true),
        tool("list_games", "The games in the user's Pious library.", json!({}), &[], true),
        tool("list_accounts", "The user's Roblox accounts in Pious.", json!({}), &[], true),
        tool("list_friends", "The user's Roblox friends, who's online and what they're playing.", json!({}), &[], true),
        tool("list_macros", "The user's macros.", json!({}), &[], true),
        tool("get_stats", "Totals Pious has kept: play time, sessions, clips and more.", json!({}), &[], true),
        tool(
            "launch_game",
            "Starts a Roblox game.",
            json!({ "game": string("Game name, Pious game ID or Roblox place ID"), "account": string("Account username or nickname (optional)"), "force": { "type": "boolean", "description": "Go ahead even if Pious would ask first" } }),
            &["game"],
            false,
        ),
        tool("join_friend", "Joins the game a friend is playing.", json!({ "friend": string("Friend's username or display name") }), &["friend"], false),
        tool("join_server_link", "Joins a private server from its link.", json!({ "link": string("A Roblox private server or share link") }), &["link"], false),
        tool("server_hop", "Moves to another public server of a game.", json!({ "game": string("Game name (optional: the running game)") }), &[], false),
        tool("close_game", "Closes one running game, or all of them.", json!({ "id": string("A running game's ID from get_status (optional: all)") }), &[], false),
        tool("toggle_recording", "Starts or stops recording the game.", json!({}), &[], false),
        tool("save_clip", "Saves the last few seconds of gameplay as a clip.", json!({ "seconds": { "type": "integer", "description": "How many seconds (optional)" } }), &[], false),
        tool("run_macro", "Runs one of the user's macros (or stops it if it's running).", json!({ "name": string("The macro's name") }), &["name"], false),
        tool("stop_macros", "Stops every macro and the auto-clicker.", json!({}), &[], false),
        tool("list_instances", "The Roblox windows running now: their game, account and server, and which one is in front.", json!({}), &[], true),
        tool(
            "screenshot",
            "A picture of what a running Roblox window shows right now (even if it's behind other windows). Use it to see the game before and after acting.",
            json!({ "instance": instance.clone(), "max_width": { "type": "integer", "description": "Shrink the picture to at most this width (default 1280)" } }),
            &[],
            true,
        ),
        tool(
            "watch",
            "Several pictures of a Roblox window over a few seconds, like a short video, to see movement.",
            json!({ "instance": instance.clone(), "seconds": { "type": "number", "description": "How long to watch (0.5–15, default 3)" }, "frames": { "type": "integer", "description": "How many pictures (2–8, default 4)" }, "max_width": { "type": "integer", "description": "Picture width (default 800)" } }),
            &[],
            true,
        ),
        tool(
            "listen",
            "Records what a Roblox game sounds like (only that game, not the rest of the PC) for a few seconds, as audio.",
            json!({ "instance": instance.clone(), "seconds": { "type": "number", "description": "How long (1–15, default 5)" } }),
            &[],
            true,
        ),
        tool("focus_instance", "Brings a Roblox window to the front.", json!({ "instance": instance.clone() }), &[], false),
        tool(
            "move_character",
            "Walks the character: holds W, A, S or D (or jumps) in a Roblox window for a while. Camera-relative, like a player.",
            json!({
                "instance": instance.clone(),
                "direction": { "type": "string", "enum": ["forward", "back", "left", "right", "forward_left", "forward_right", "jump"] },
                "seconds": { "type": "number", "description": "How long to hold it (default 1)" },
                "jump": { "type": "boolean", "description": "Jump at the start too" },
            }),
            &[],
            false,
        ),
        tool(
            "press_keys",
            "Presses keys in a Roblox window: one key (\"e\"), or a combination held together ([\"shift\", \"w\"]). Names like w, space, shift, ctrl, enter, f5, 1, or KeyboardEvent codes like KeyE.",
            json!({
                "instance": instance.clone(),
                "keys": { "description": "A key, \"shift+w\", or a list held together", "anyOf": [{ "type": "string" }, { "type": "array", "items": { "type": "string" } }] },
                "hold_ms": { "type": "integer", "description": "How long they're held (default 60)" },
                "times": { "type": "integer", "description": "How many times (default 1)" },
            }),
            &["keys"],
            false,
        ),
        tool(
            "type_text",
            "Types text in a Roblox window. With chat (the default) it opens Roblox's chat with /, types, and sends with Enter.",
            json!({ "instance": instance.clone(), "text": string("What to type"), "chat": { "type": "boolean", "description": "Send it as a chat message (default true)" }, "enter": { "type": "boolean", "description": "Press Enter after (when chat is false)" } }),
            &["text"],
            false,
        ),
        tool(
            "click",
            "Clicks in a Roblox window at a spot given as fractions of the window (0,0 top left; 1,1 bottom right). Take a screenshot first to aim.",
            json!({
                "instance": instance.clone(),
                "x": { "type": "number", "description": "0–1 across (default 0.5)" },
                "y": { "type": "number", "description": "0–1 down (default 0.5)" },
                "button": { "type": "string", "enum": ["left", "right", "middle"] },
                "count": { "type": "integer", "description": "Clicks (default 1; 2 for a double click)" },
            }),
            &[],
            false,
        ),
        tool(
            "run_input",
            "Plays a list of input steps in a Roblox window, for anything the other tools don't cover. Steps: {\"kind\":\"Key\",\"key\":\"KeyE\",\"press\":\"Tap\"|\"Down\"|\"Up\",\"hold_ms\":50}, {\"kind\":\"Text\",\"text\":\"hi\"}, {\"kind\":\"Click\",\"button\":\"Left\",\"press\":\"Tap\",\"count\":1}, {\"kind\":\"Scroll\",\"amount\":-3}, {\"kind\":\"Wait\",\"ms\":500}, {\"kind\":\"Loop\",\"times\":3,\"steps\":[…]}.",
            json!({ "instance": instance, "steps": { "type": "array", "items": { "type": "object" } } }),
            &["steps"],
            false,
        ),
        tool(
            "send_message",
            "Sends a Roblox chat message to a friend.",
            json!({ "friend": string("Friend's username or display name"), "text": string("The message") }),
            &["friend", "text"],
            false,
        ),
    ]
}

/// `pious.exe --mcp`: relays an AI app's stdio to the running Pious.
///
/// AI apps start every MCP server they know about as soon as they open (a
/// Claude Code session, Cursor…), so this must never start Pious by itself
/// just for that. While Pious isn't reachable, the handshake and the tool
/// list are answered here, and only an actual tool call can start Pious,
/// and only if the user allowed it (`mcp.start_on_demand`).
pub fn stdio_bridge() {
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(_) => return,
    };
    runtime.block_on(async {
        // Read without touching the data folder's layout (no migration here:
        // the real Pious may be running and owns that).
        let prefs = std::fs::read(store::data_file_readonly())
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .map(|v| v["preferences"]["mcp"].clone())
            .unwrap_or(Value::Null);
        let port = prefs["port"].as_u64().unwrap_or(47_823);
        let start_on_demand = prefs["start_on_demand"].as_bool().unwrap_or(false);
        let url = format!("http://127.0.0.1:{port}/mcp");
        let client = reqwest::Client::builder().connect_timeout(Duration::from_millis(800)).build().unwrap_or_default();
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut lines = BufReader::new(stdin).lines();
        let mut started = false;
        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }
            let message = serde_json::from_str::<Value>(&line).unwrap_or(Value::Null);
            let method = message["method"].as_str().unwrap_or_default().to_owned();
            let forward = |secret: Option<String>| {
                let mut request = client.post(&url).header("Content-Type", "application/json").body(line.clone());
                if let Some(secret) = secret {
                    request = request.bearer_auth(secret);
                }
                request.send()
            };
            // Read each time: Pious may have made (or renewed) its token since
            // this bridge started. Refused for the token means the same as
            // unreachable to the AI app.
            let answer = |response: reqwest::Response| async move {
                if response.status() == reqwest::StatusCode::UNAUTHORIZED { None } else { Some(response.text().await.unwrap_or_default()) }
            };
            let mut reply = match forward(token_readonly()).await {
                Ok(response) => answer(response).await,
                Err(_) => None,
            };
            // Only a real tool call may wake Pious, and only when allowed.
            if reply.is_none() && method == "tools/call" && start_on_demand && !started {
                started = true;
                if let Ok(exe) = std::env::current_exe() {
                    let _ = std::process::Command::new(exe).arg("--background").spawn();
                }
                for _ in 0..30 {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    // The token may have just been made by the new Pious.
                    if let Ok(response) = forward(token_readonly()).await {
                        reply = answer(response).await;
                        if reply.is_some() {
                            break;
                        }
                    }
                }
            }
            let reply = reply.or_else(|| offline_reply(&message, start_on_demand).map(|v| v.to_string()));
            if let Some(reply) = reply.filter(|r| !r.trim().is_empty()) {
                let _ = stdout.write_all(reply.trim().as_bytes()).await;
                let _ = stdout.write_all(b"\n").await;
                let _ = stdout.flush().await;
            }
        }
    });
}

/// The token, if Pious has made one (never creates it or the data folder).
fn token_readonly() -> Option<String> {
    let text = std::fs::read_to_string(store::data_dir_readonly().join("mcp-token.txt")).ok()?;
    Some(text.trim().to_owned()).filter(|t| t.len() >= 32)
}

/// What the bridge answers by itself while Pious isn't reachable, so the AI
/// app connects at once (instead of timing out) without Pious starting.
fn offline_reply(message: &Value, start_on_demand: bool) -> Option<Value> {
    let id = message.get("id").cloned()?;
    let params = &message["params"];
    let not_running = if start_on_demand {
        "Pious couldn't be started. Open Pious and turn on \"Let AI apps use Pious\" in Settings → Plugins."
    } else {
        "Pious isn't running (or \"Let AI apps use Pious\" is off). Open Pious first; it isn't started automatically unless \"Start Pious when an AI app needs it\" is on in Settings → Plugins."
    };
    Some(match message["method"].as_str().unwrap_or_default() {
        "initialize" => json!({ "jsonrpc": "2.0", "id": id, "result": {
            "protocolVersion": params["protocolVersion"].as_str().unwrap_or(PROTOCOL),
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "pious", "title": "Pious", "version": crate::core::updater::CURRENT },
            "instructions": "Pious is a Roblox launcher. It isn't running right now; tool calls will say so until the user opens it.",
        } }),
        "ping" => json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
        "tools/list" => json!({ "jsonrpc": "2.0", "id": id, "result": { "tools": tools() } }),
        "tools/call" => json!({ "jsonrpc": "2.0", "id": id, "result": {
            "content": [{ "type": "text", "text": not_running }],
            "isError": true,
        } }),
        method => rpc_error(id, -32601, &format!("Unknown method {method}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_bridge_answers_without_pious() {
        let init = offline_reply(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } }), false).unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "pious");
        let list = offline_reply(&json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }), false).unwrap();
        assert!(list["result"]["tools"].as_array().is_some_and(|t| !t.is_empty()));
        let call = offline_reply(&json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "get_status" } }), false).unwrap();
        assert_eq!(call["result"]["isError"], true);
        // Notifications get no answer.
        assert!(offline_reply(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }), false).is_none());
    }
}
