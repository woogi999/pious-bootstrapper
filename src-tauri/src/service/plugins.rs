//! Plugins: folders in `<data>\plugins`, each with a `plugin.json` and
//! (usually) a page Pious shows in its sidebar. A plugin page runs walled
//! off from Pious and talks to it only through messages, limited to the
//! permissions its manifest asks for (see `pious-plugin.js`).

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::core::store;

pub fn dir() -> PathBuf {
    store::data_dir().join("plugins")
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Manifest {
    id: String,
    name: String,
    version: String,
    author: String,
    description: String,
    /// An HTML page in the plugin's folder, shown in Pious.
    page: Option<String>,
    /// One of Pious's icon names.
    icon: Option<String>,
    permissions: Vec<String>,
}

/// A plugin as the window sees it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    /// The page file, when it has one.
    pub page: Option<PathBuf>,
    pub icon: String,
    pub permissions: Vec<String>,
    pub enabled: bool,
    pub folder: PathBuf,
    /// Why it can't be used, if it can't.
    pub problem: Option<String>,
    /// Comes with Pious (its page is part of the app, not a folder).
    pub builtin: bool,
}

/// The plugins that come with Pious, off until turned on.
fn builtins() -> Vec<Plugin> {
    vec![Plugin {
        id: crate::core::model::MACROS_PLUGIN.into(),
        name: "Macros".into(),
        version: crate::core::updater::CURRENT.into(),
        author: "Pious".into(),
        description: "Record or build macros and run them with a hotkey, plus an auto-clicker. Both can send their keys and clicks to Roblox in the background, so you can keep using your PC.".into(),
        page: None,
        icon: "macros".into(),
        permissions: Vec::new(),
        enabled: false,
        folder: PathBuf::new(),
        problem: None,
        builtin: true,
    }]
}

/// What each permission lets a plugin do, in words.
pub const PERMISSIONS: [(&str, &str); 6] = [
    ("read", "See your games, accounts, friends and running games"),
    ("launch", "Start and join games"),
    ("notify", "Show notices"),
    ("navigate", "Open Pious pages"),
    ("macros", "Run your macros"),
    ("links", "Open web links"),
];

/// The built-in plugins and `plugins` (from [`list`]), with whether each
/// is turned on now.
pub fn with_enabled(plugins: &[Plugin], enabled: &BTreeSet<String>) -> Vec<Plugin> {
    builtins()
        .iter()
        .chain(plugins.iter().filter(|p| !p.builtin))
        .map(|p| Plugin { enabled: enabled.contains(&p.id) && p.problem.is_none(), ..p.clone() })
        .collect()
}

pub fn list(enabled: &BTreeSet<String>) -> Vec<Plugin> {
    let Ok(entries) = std::fs::read_dir(dir()) else { return Vec::new() };
    let mut plugins: Vec<Plugin> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|entry| {
            let folder = entry.path();
            let fallback = entry.file_name().to_string_lossy().into_owned();
            let parsed = std::fs::read(folder.join("plugin.json"))
                .map_err(|_| "It has no plugin.json.".to_owned())
                .and_then(|bytes| serde_json::from_slice::<Manifest>(&bytes).map_err(|e| format!("Its plugin.json doesn't read ({e}).")));
            let (manifest, mut problem) = match parsed {
                Ok(m) => (m, None),
                Err(problem) => (Manifest::default(), Some(problem)),
            };
            let id = if manifest.id.trim().is_empty() { fallback.clone() } else { manifest.id.trim().to_owned() };
            let page = manifest.page.as_deref().map(|p| folder.join(p));
            if let Some(page) = &page {
                // Pages must stay inside their own folder.
                let inside = page.canonicalize().ok().zip(folder.canonicalize().ok()).is_some_and(|(p, f)| p.starts_with(f));
                if !inside && problem.is_none() {
                    problem = Some("Its page is missing.".into());
                }
            }
            let mut permissions: Vec<String> =
                manifest.permissions.into_iter().filter(|p| PERMISSIONS.iter().any(|(k, _)| k == p)).collect();
            permissions.dedup();
            Plugin {
                enabled: enabled.contains(&id) && problem.is_none(),
                name: if manifest.name.trim().is_empty() { fallback } else { manifest.name },
                id,
                version: manifest.version,
                author: manifest.author,
                description: manifest.description,
                page,
                icon: manifest.icon.unwrap_or_else(|| "puzzle".into()),
                permissions,
                folder,
                problem,
                builtin: false,
            }
        })
        .collect();
    plugins.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    plugins
}

const SDK: &str = r#"// The Pious plugin API. A plugin page loads this file first:
//
//   <script src="pious-plugin.js"></script>
//
// then uses the global `pious`:
//
//   const snap = await pious.call("snapshot");      // needs "read"
//   await pious.call("toast", "Hello from my plugin"); // needs "notify"
//   pious.on("snapshot", (snap) => { ... });         // live updates ("read")
//
// Methods: snapshot, launch(gameId), join(placeId), toast(text),
// navigate(page), runMacro(name), openLink(url). Each needs the matching
// permission in plugin.json ("read", "launch", "notify", "navigate",
// "macros", "links"). The page gets Pious's colors as CSS variables
// (--bg, --text, --accent, --surface, as "r g b") and the class "pious".

(() => {
const waiting = new Map();
const listeners = new Map();
let next = 0;

window.addEventListener("message", (event) => {
  const message = event.data;
  if (!message || typeof message !== "object" || event.source !== window.parent) return;
  if (message.pious === "reply" && waiting.has(message.id)) {
    const { resolve, reject } = waiting.get(message.id);
    waiting.delete(message.id);
    if (message.error) reject(new Error(message.error));
    else resolve(message.value);
  } else if (message.pious === "theme") {
    const root = document.documentElement;
    root.classList.add("pious");
    for (const [name, value] of Object.entries(message.vars)) root.style.setProperty(name, value);
  } else if (message.pious === "event") {
    for (const listener of listeners.get(message.event) ?? []) listener(message.value);
  }
});

window.pious = {
  call(method, ...args) {
    const id = ++next;
    return new Promise((resolve, reject) => {
      waiting.set(id, { resolve, reject });
      window.parent.postMessage({ pious: "call", id, method, args }, "*");
    });
  },
  on(event, listener) {
    if (!listeners.has(event)) listeners.set(event, new Set());
    listeners.get(event).add(listener);
    window.parent.postMessage({ pious: "subscribe", event }, "*");
    return () => listeners.get(event)?.delete(listener);
  },
};

window.parent.postMessage({ pious: "ready" }, "*");
})();
"#;

const EXAMPLE_MANIFEST: &str = r#"{
  "id": "example",
  "name": "Example plugin",
  "version": "1.0.0",
  "author": "You",
  "description": "A starting point: shows your running games and friends who are playing.",
  "page": "index.html",
  "icon": "puzzle",
  "permissions": ["read", "notify", "launch"]
}
"#;

const EXAMPLE_PAGE: &str = r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <style>
      body { margin: 0; padding: 24px; font: 14px system-ui, sans-serif; color: rgb(var(--text, 237 237 239)); background: transparent; }
      h1 { font-size: 22px; margin: 0 0 4px; }
      .muted { opacity: 0.6; }
      .card { margin-top: 12px; padding: 12px 14px; border-radius: 12px; background: rgb(var(--surface, 255 255 255) / 0.06); border: 1px solid rgb(var(--surface, 255 255 255) / 0.08); animation: rise 260ms ease; }
      button { font: inherit; padding: 6px 12px; border-radius: 8px; border: none; background: rgb(var(--accent, 242 242 243)); color: rgb(var(--bg, 10 10 11)); cursor: pointer; }
      @keyframes rise { from { opacity: 0; transform: translateY(6px); } }
    </style>
  </head>
  <body>
    <h1>Example plugin</h1>
    <p class="muted">Edit index.html in this plugin's folder to make it yours.</p>
    <button id="hello">Say hello</button>
    <div id="out"></div>
    <script src="pious-plugin.js"></script>
    <script>
      const out = document.getElementById("out");
      const draw = (snap) => {
        const playing = snap.friends.filter((f) => f.status === "in_game");
        out.innerHTML = `
          <div class="card"><b>${snap.running.length}</b> game(s) running</div>
          <div class="card"><b>${playing.length}</b> friend(s) playing${playing.length ? ": " + playing.map((f) => f.display_name).join(", ") : ""}</div>`;
      };
      pious.call("snapshot").then(draw);
      pious.on("snapshot", draw);
      document.getElementById("hello").onclick = () => pious.call("toast", "Hello from the example plugin!");
    </script>
  </body>
</html>
"#;

/// Writes the example plugin (and the API file) to start from.
pub fn create_example() -> Result<PathBuf, String> {
    let folder = dir().join("example");
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let write = |name: &str, text: &str| -> Result<(), String> {
        let path = folder.join(name);
        if name == "pious-plugin.js" || !path.exists() {
            std::fs::write(path, text).map_err(|e| e.to_string())?;
        }
        Ok(())
    };
    write("plugin.json", EXAMPLE_MANIFEST)?;
    write("index.html", EXAMPLE_PAGE)?;
    write("pious-plugin.js", SDK)?;
    Ok(folder)
}

/// Keeps every plugin's copy of the API file current.
pub fn refresh_sdk() {
    for plugin in list(&BTreeSet::new()) {
        let path = plugin.folder.join("pious-plugin.js");
        if path.exists() {
            let _ = std::fs::write(path, SDK);
        }
    }
}
