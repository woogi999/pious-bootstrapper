//! What plugins can ask of Pious: the capabilities a plugin's page and its
//! background engine use (input, hotkeys, Roblox windows, timing, storage,
//! settings…), requests Pious sends to plugins (like "run this macro"), and
//! the hidden window engines run in. See docs/PLUGIN-API.md.
//!
//! The page and the engine run walled off in frames; their host (a Pious
//! window) passes each call here with the plugin's ID, which the frame
//! can't choose itself. Every call is checked against the permissions the
//! plugin's manifest asked for.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use super::plugins::Plugin;
use super::{Service, Shared, Tone};
use crate::core::automation::{self, caps};
use crate::core::process;

/// The hidden window plugins' engines run in.
pub const HOST_WINDOW: &str = "plugin-host";

/// What a plugin is doing, shown as a chip in the top bar.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PluginStatus {
    pub plugin: String,
    pub name: String,
    pub text: String,
    pub active: bool,
}

/// Plugin state that isn't saved.
#[derive(Default)]
pub struct Runtime {
    /// Hotkeys plugins asked for: (plugin, its hotkey ID, keys, only in game).
    pub hotkeys: Vec<(String, String, String, bool)>,
    pub status: Vec<PluginStatus>,
    /// Stylesheets plugins added (`ui.css`), by plugin.
    pub css: std::collections::BTreeMap<String, String>,
    /// Plugins following the real keyboard and mouse (`input.watch`).
    pub watching: std::collections::BTreeSet<String>,
    /// Plugins recording the keyboard and mouse.
    pub recording: Option<String>,
}

/// Requests waiting for an engine's answer.
static PENDING: Mutex<Option<HashMap<u64, tokio::sync::oneshot::Sender<Result<Value, String>>>>> = Mutex::new(None);
static NEXT: AtomicU64 = AtomicU64::new(1);

/// The permission each method needs (`""`: none).
fn needs(method: &str) -> Option<&'static str> {
    Some(match method {
        "sleep" | "storage.read" | "storage.write" | "storage.list" | "storage.remove" | "send" | "status" | "respond" => "",
        "hud" => "notify",
        m if m.starts_with("input.") || m.starts_with("windows.") || m == "idle" => "input",
        "hotkeys.set" => "hotkeys",
        "system.open" => "run",
        "settings.get" | "settings.set" | "ui.css" => "full",
        _ => return None,
    })
}

fn arg<'a>(args: &'a [Value], i: usize) -> &'a Value {
    args.get(i).unwrap_or(&Value::Null)
}

fn text_arg(args: &[Value], i: usize) -> String {
    match arg(args, i) {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn target(args: &[Value], i: usize) -> Option<isize> {
    arg(args, i).as_i64().map(|w| w as isize)
}

fn point(value: &Value) -> Option<(i32, i32)> {
    Some((value.get("x")?.as_f64()? as i32, value.get("y")?.as_f64()? as i32))
}

/// A storage file name that stays in the plugin's `data` folder.
fn storage_path(plugin: &Plugin, name: &str) -> Result<PathBuf, String> {
    let ok = !name.is_empty() && name.len() <= 100 && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')) && !name.starts_with('.');
    if !ok {
        return Err(format!("{name} isn't a name a plugin can save under (letters, digits, '.', '-', '_')."));
    }
    Ok(plugin.folder.join("data").join(name))
}

impl Service {
    fn enabled_plugin(&self, id: &str) -> Result<Plugin, String> {
        let s = self.read();
        let plugin = super::plugins::with_enabled(&s.plugins, &s.bootstrapper.preferences.plugins)
            .into_iter()
            .find(|p| p.id == id)
            .ok_or("That plugin isn't installed.")?;
        if !plugin.enabled {
            return Err(format!("{} is turned off.", plugin.name));
        }
        Ok(plugin)
    }

    /// One call from a plugin (`side`: "page" or "engine").
    pub async fn plugin_call(self: &Shared, id: &str, side: &str, method: &str, args: Vec<Value>) -> Result<Value, String> {
        let plugin = self.enabled_plugin(id)?;
        let needed = needs(method).ok_or_else(|| format!("Pious has no {method}."))?;
        if !needed.is_empty() && !plugin.permissions.iter().any(|p| p == needed || p == "full") {
            return Err(format!("{} didn't ask for the {needed} permission.", plugin.name));
        }
        match method {
            "sleep" => {
                let ms = arg(&args, 0).as_f64().unwrap_or(0.0).clamp(0.0, 3_600_000.0);
                tokio::time::sleep(Duration::from_secs_f64(ms / 1000.0)).await;
                Ok(Value::Null)
            }
            "storage.read" => {
                let path = storage_path(&plugin, &text_arg(&args, 0))?;
                Ok(tokio::fs::read_to_string(path).await.map(Value::String).unwrap_or(Value::Null))
            }
            "storage.write" => {
                let path = storage_path(&plugin, &text_arg(&args, 0))?;
                let body = text_arg(&args, 1);
                if body.len() > 32 * 1024 * 1024 {
                    return Err("That's too much to save (32 MB at most).".into());
                }
                tokio::task::spawn_blocking(move || -> Result<(), String> {
                    std::fs::create_dir_all(path.parent().ok_or("No folder")?).map_err(|e| e.to_string())?;
                    // Written whole, then swapped in.
                    let partial = path.with_extension("part");
                    std::fs::write(&partial, body).map_err(|e| e.to_string())?;
                    std::fs::rename(&partial, &path).map_err(|e| e.to_string())
                })
                .await
                .map_err(|e| e.to_string())??;
                Ok(Value::Bool(true))
            }
            "storage.list" => {
                let dir = plugin.folder.join("data");
                let mut names: Vec<String> = std::fs::read_dir(dir)
                    .map(|e| e.flatten().filter(|e| e.path().is_file()).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
                    .unwrap_or_default();
                names.sort();
                Ok(json!(names))
            }
            "storage.remove" => {
                let path = storage_path(&plugin, &text_arg(&args, 0))?;
                let _ = std::fs::remove_file(path);
                Ok(Value::Bool(true))
            }
            "send" => {
                let to = if side == "engine" { "page" } else { "engine" };
                self.plugin_event(id, to, "message", arg(&args, 0).clone());
                Ok(Value::Bool(true))
            }
            "status" => {
                let value = arg(&args, 0);
                self.mutate(|s| {
                    s.plugin_runtime.status.retain(|st| st.plugin != id);
                    if let Some(text) = value.get("text").and_then(Value::as_str).filter(|t| !t.trim().is_empty()) {
                        s.plugin_runtime.status.push(PluginStatus {
                            plugin: id.to_owned(),
                            name: plugin.name.clone(),
                            text: text.chars().take(60).collect(),
                            active: value.get("active").and_then(Value::as_bool).unwrap_or(true),
                        });
                    }
                });
                Ok(Value::Bool(true))
            }
            "respond" => {
                let rid = arg(&args, 0).as_u64().ok_or("Which request?")?;
                let error = arg(&args, 2).as_str().map(str::to_owned);
                let answer = match error {
                    Some(error) => Err(error),
                    None => Ok(arg(&args, 1).clone()),
                };
                if let Some(sender) = PENDING.lock().ok().and_then(|mut p| p.as_mut()?.remove(&rid)) {
                    let _ = sender.send(answer);
                }
                Ok(Value::Bool(true))
            }
            "hud" => {
                self.hud(&text_arg(&args, 0), &text_arg(&args, 1).chars().take(120).collect::<String>());
                Ok(Value::Bool(true))
            }
            // ── Input ────────────────────────────────────────────────────
            "input.key" => {
                let (code, down, t) = (text_arg(&args, 0), arg(&args, 1).as_bool().unwrap_or(true), target(&args, 2));
                tokio::task::spawn_blocking(move || caps::key(t, &code, down)).await.map_err(|e| e.to_string())??;
                Ok(Value::Null)
            }
            "input.text" => {
                let (text, t) = (text_arg(&args, 0), target(&args, 1));
                tokio::task::spawn_blocking(move || caps::text(t, &text)).await.map_err(|e| e.to_string())?;
                Ok(Value::Null)
            }
            "input.button" => {
                let name = text_arg(&args, 0);
                let button = automation::button_of(&name).ok_or(format!("{name} isn't a mouse button."))?;
                let (down, t, at) = (arg(&args, 1).as_bool().unwrap_or(true), target(&args, 2), point(arg(&args, 3)));
                tokio::task::spawn_blocking(move || caps::button(t, button, down, at)).await.map_err(|e| e.to_string())?;
                Ok(Value::Null)
            }
            "input.move" => {
                let (x, y) = (arg(&args, 0).as_f64().unwrap_or(0.0) as i32, arg(&args, 1).as_f64().unwrap_or(0.0) as i32);
                let (relative, t) = (arg(&args, 2).as_bool().unwrap_or(false), target(&args, 3));
                tokio::task::spawn_blocking(move || caps::move_pointer(t, x, y, relative)).await.map_err(|e| e.to_string())?;
                Ok(Value::Null)
            }
            "input.scroll" => {
                let (n, horizontal) = (arg(&args, 0).as_i64().unwrap_or(0) as i32, arg(&args, 1).as_bool().unwrap_or(false));
                let (t, at) = (target(&args, 2), point(arg(&args, 3)));
                tokio::task::spawn_blocking(move || caps::scroll(t, n, horizontal, at)).await.map_err(|e| e.to_string())?;
                Ok(Value::Null)
            }
            "input.cursor" => {
                let (x, y) = automation::cursor();
                Ok(json!({ "x": x, "y": y }))
            }
            "input.pixel" => {
                let (x, y, t) = (arg(&args, 0).as_f64().unwrap_or(0.0) as i32, arg(&args, 1).as_f64().unwrap_or(0.0) as i32, target(&args, 2));
                Ok(tokio::task::spawn_blocking(move || caps::pixel(t, x, y)).await.map_err(|e| e.to_string())?.map(Value::String).unwrap_or(Value::Null))
            }
            "input.windowPoint" => {
                let window = arg(&args, 0).as_i64().ok_or("Which window?")? as isize;
                let (x, y) = automation::window_point(window, arg(&args, 1).as_f64().unwrap_or(0.5), arg(&args, 2).as_f64().unwrap_or(0.5));
                Ok(json!({ "x": x, "y": y }))
            }
            "input.watch" => {
                let on = arg(&args, 0).as_bool().unwrap_or(false);
                self.mutate(|s| {
                    if on {
                        s.plugin_runtime.watching.insert(id.to_owned());
                    } else {
                        s.plugin_runtime.watching.remove(id);
                    }
                    s.input_dirty = true;
                });
                Ok(Value::Bool(true))
            }
            "input.recordStart" => {
                if self.read().plugin_runtime.recording.is_some() {
                    return Err("Something is already recording.".into());
                }
                automation::start_recording()?;
                self.mutate(|s| s.plugin_runtime.recording = Some(id.to_owned()));
                Ok(Value::Bool(true))
            }
            "input.recordStop" => {
                if self.read().plugin_runtime.recording.as_deref() != Some(id) {
                    return Ok(json!([]));
                }
                let events = automation::stop_recording();
                self.mutate(|s| s.plugin_runtime.recording = None);
                Ok(Value::Array(automation::events_json(&events)))
            }
            "windows.roblox" => Ok(json!(self.roblox_windows().await)),
            "windows.focus" => {
                let window = arg(&args, 0).as_i64().ok_or("Which window?")? as isize;
                Ok(Value::Bool(tokio::task::spawn_blocking(move || process::bring_to_front(window)).await.unwrap_or(false)))
            }
            "windows.foreground" => {
                let window = process::foreground_window();
                let pid = process::window_pid(window);
                let roblox = self.read().foreground_roblox == Some(pid);
                Ok(json!({ "window": window, "pid": pid, "roblox": roblox }))
            }
            "idle" => Ok(json!(process::idle_seconds())),
            "system.open" => {
                let target = text_arg(&args, 0);
                if target.trim().is_empty() {
                    return Err("Say what to open.".into());
                }
                open::that_detached(target.trim()).map_err(|e| format!("Couldn't open {target} ({e})."))?;
                Ok(Value::Bool(true))
            }
            // ── Hotkeys ──────────────────────────────────────────────────
            "hotkeys.set" => {
                let list: Vec<(String, String, String, bool)> = arg(&args, 0)
                    .as_array()
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|h| {
                                let keys = h.get("keys")?.as_str()?.trim().to_owned();
                                let hid = h.get("id").map(|v| v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string()))?;
                                (!keys.is_empty()).then(|| (id.to_owned(), hid, keys, h.get("gameOnly").and_then(Value::as_bool).unwrap_or(false)))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                self.read().plugin_runtime.hotkeys.retain(|(p, ..)| p != id);
                self.read().plugin_runtime.hotkeys.extend(list);
                Ok(json!(self.apply_hotkeys_report()))
            }
            // ── Full access ──────────────────────────────────────────────
            "settings.get" => Ok(serde_json::to_value(&self.read().bootstrapper.preferences).map_err(|e| e.to_string())?),
            "settings.set" => {
                self.update_preferences(arg(&args, 0).clone()).await?;
                Ok(Value::Bool(true))
            }
            "ui.css" => {
                let css = text_arg(&args, 0);
                self.read().plugin_runtime.css.insert(id.to_owned(), css);
                self.emit_theme_assets();
                Ok(Value::Bool(true))
            }
            _ => Err(format!("Pious has no {method}.")),
        }
    }

    /// The Roblox windows, for plugins.
    async fn roblox_windows(&self) -> Vec<Value> {
        let (instances, front, last) = {
            let s = self.read();
            let lib = &s.bootstrapper;
            let instances: Vec<(u32, Option<(uuid::Uuid, String)>, Option<String>)> = s
                .instances
                .iter()
                .filter_map(|i| {
                    let account = i.account.and_then(|a| lib.account(a)).map(|a| (a.id, a.username.clone()));
                    Some((i.pid?, account, lib.game(i.game).map(|g| g.name.clone())))
                })
                .collect();
            (instances, s.foreground_roblox, s.last_roblox_window)
        };
        let pids = tokio::task::spawn_blocking(process::roblox_pids).await.unwrap_or_default();
        pids.into_iter()
            .filter_map(|pid| {
                let window = process::window_of(pid)?;
                let known = instances.iter().find(|(p, ..)| *p == pid);
                Some(json!({
                    "window": window,
                    "pid": pid,
                    "account": known.and_then(|(_, a, _)| a.as_ref()).map(|(id, name)| json!({ "id": id, "username": name })),
                    "game": known.and_then(|(_, _, g)| g.clone()),
                    "front": front == Some(pid),
                    "last": last == Some(window),
                }))
            })
            .collect()
    }

    /// Sends an event to a plugin's page, engine or both ("page",
    /// "engine", "both").
    pub fn plugin_event(&self, plugin: &str, to: &str, event: &str, value: Value) {
        let _ = self.app().emit("plugin-event", json!({ "plugin": plugin, "to": to, "event": event, "value": value }));
    }

    /// Asks the engine of the plugin providing `feature` (e.g. "macros")
    /// for something, and waits for its answer.
    pub async fn plugin_request(self: &Shared, feature: &str, method: &str, args: Value) -> Result<Value, String> {
        let plugin = {
            let s = self.read();
            super::plugins::with_enabled(&s.plugins, &s.bootstrapper.preferences.plugins)
                .into_iter()
                .find(|p| p.enabled && p.provides.as_deref() == Some(feature) && p.main.is_some())
        };
        let Some(plugin) = plugin else {
            return Err(match feature {
                "macros" => "Macros are a plugin that's off. Turn it on in Settings → Plugins.".into(),
                _ => format!("No plugin that does {feature} is on."),
            });
        };
        let rid = NEXT.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = tokio::sync::oneshot::channel();
        if let Ok(mut pending) = PENDING.lock() {
            pending.get_or_insert_with(HashMap::new).insert(rid, sender);
        }
        self.plugin_event(&plugin.id, "engine", "request", json!({ "rid": rid, "method": method, "args": args }));
        let answer = tokio::time::timeout(Duration::from_secs(15), receiver).await;
        if let Ok(mut pending) = PENDING.lock() {
            if let Some(p) = pending.as_mut() {
                p.remove(&rid);
            }
        }
        match answer {
            Ok(Ok(answer)) => answer,
            _ => Err(format!("{} didn't answer. Is it running? (Settings → Plugins)", plugin.name)),
        }
    }

    /// Keeps the hidden window plugins' engines run in: made while any
    /// enabled plugin has an engine, closed when none has (it costs memory).
    pub fn sync_plugin_host(self: &Shared) {
        let wanted = {
            let s = self.read();
            super::plugins::with_enabled(&s.plugins, &s.bootstrapper.preferences.plugins).iter().any(|p| p.enabled && p.main.is_some())
        };
        let app = self.app().clone();
        let me = self.clone();
        tauri::async_runtime::spawn_blocking(move || {
            // One check-and-build at a time: two syncs at once (startup and
            // the disk loop) would otherwise both make a host, and every
            // engine would run twice.
            static BUILDING: std::sync::Mutex<()> = std::sync::Mutex::new(());
            let _turn = BUILDING.lock().unwrap_or_else(|e| e.into_inner());
            let existing = app.get_webview_window(HOST_WINDOW);
            match (wanted, existing) {
                (true, None) => {
                    let built = WebviewWindowBuilder::new(&app, HOST_WINDOW, WebviewUrl::App("index.html".into()))
                        .title("Pious plugins")
                        .visible(false)
                        .skip_taskbar(true)
                        .focused(false)
                        .build();
                    if let Err(error) = built {
                        me.toast(Tone::Negative, format!("Plugins' engines couldn't start ({error})."));
                    }
                }
                (false, Some(window)) => {
                    let _ = window.destroy();
                }
                _ => {}
            }
        });
        // Also tells an open host which engines to run now.
        let _ = self.app().emit_to(HOST_WINDOW, "plugin-engines-changed", ());
    }

    /// A key or mouse button the input hook saw (for plugins that watch).
    pub(super) fn plugin_input(&self, code: &str, down: bool) {
        let watching: Vec<String> = self.read().plugin_runtime.watching.iter().cloned().collect();
        for plugin in watching {
            self.plugin_event(&plugin, "engine", "input", json!({ "code": code, "down": down }));
        }
    }
}

/// Where a plugin's engine page is (Pious writes it next to the engine:
/// it loads the API file, then the engine script).
pub fn engine_page(plugin: &Plugin) -> Option<PathBuf> {
    plugin.main.as_ref().map(|_| plugin.folder.join("pious-engine.html"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn methods_need_their_permissions() {
        assert_eq!(needs("sleep"), Some(""));
        assert_eq!(needs("input.key"), Some("input"));
        assert_eq!(needs("windows.roblox"), Some("input"));
        assert_eq!(needs("hotkeys.set"), Some("hotkeys"));
        assert_eq!(needs("settings.set"), Some("full"));
        assert_eq!(needs("nonsense"), None);
    }

    #[test]
    fn storage_names_stay_inside() {
        let plugin = Plugin {
            folder: PathBuf::from("C:/plugins/x"),
            ..super::super::plugins::tests::blank()
        };
        assert!(storage_path(&plugin, "macros.json").is_ok());
        for bad in ["../x", "a/b", "", ".hidden", "c:x"] {
            assert!(storage_path(&plugin, bad).is_err(), "{bad}");
        }
    }
}
