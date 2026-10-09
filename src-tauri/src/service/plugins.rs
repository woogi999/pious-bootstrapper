//! Plugins: folders in the plugins folder (next to Pious when installed),
//! each with a `plugin.json`. A plugin can bring:
//!
//! - a page Pious shows in its sidebar. It runs walled off from Pious and
//!   talks to it only through messages, limited to the permissions its
//!   manifest asks for (see `pious-plugin.js`);
//! - Roblox client files: cursors, the shift lock cursor, the death sound,
//!   any sound or texture (put in place with the tweaks, undone with them);
//! - themes, interface CSS, icons and sounds for Pious itself;
//! - a feature built into Pious (`provides`), like Macros: the engine is
//!   part of Pious (it needs low-level input hooks), but the feature only
//!   exists while its plugin folder is there and turned on.
//!
//! A plugin that can't be read never stops Pious: it's listed with the
//! reason, and nothing from it is used. See `docs/PLUGINS.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::core::store;

pub fn dir() -> PathBuf {
    store::plugins_dir()
}

/// Every folder plugins are read from: the plugins folder, and in
/// development builds the repository's own `plugins` (what releases ship).
pub fn dirs() -> Vec<PathBuf> {
    let mut out = vec![dir()];
    if cfg!(debug_assertions) {
        let repo = repo_root().join("plugins");
        if repo.is_dir() {
            out.push(repo);
        }
    }
    out
}

/// The repository folder (development builds). Never `src-tauri\..`: the
/// asset scope doesn't match paths with `..` in them, so pages load blank.
pub fn repo_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().unwrap_or(manifest).to_path_buf()
}

/// The native features plugins can turn on.
pub const FEATURES: [&str; 1] = ["macros"];

/// A plugin providing the Macros feature is in the plugins folder (kept
/// fresh by [`list`], so checks never touch the disk).
static MACROS_PRESENT: AtomicBool = AtomicBool::new(false);

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
    /// A small HTML page shown as a panel in the in-game overlay.
    overlay: Option<String>,
    /// A script that runs in the background while the plugin is on.
    main: Option<String>,
    /// One of Pious's icon names.
    icon: Option<String>,
    permissions: Vec<String>,
    /// A feature built into Pious this plugin turns on ("macros").
    provides: Option<String>,
    /// A folder laid out like a Roblox version folder; every file in it
    /// replaces Roblox's at the same path.
    client: Option<String>,
    /// Roblox file (path in a version folder) → file in the plugin.
    replace: BTreeMap<String, String>,
    /// Shortcuts for the common ones.
    death_sound: Option<String>,
    cursor: Option<String>,
    cursor_far: Option<String>,
    shiftlock: Option<String>,
    /// Folders (in the plugin) holding a `theme.json` each.
    themes: Vec<String>,
    /// A stylesheet applied to Pious's own windows.
    css: Option<String>,
    /// Pious icon name → an SVG or PNG in the plugin.
    icons: BTreeMap<String, String>,
    /// Pious's own sounds ("notification") → a sound file in the plugin.
    sounds: BTreeMap<String, String>,
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
    /// Its panel for the in-game overlay, when it has one.
    pub overlay: Option<PathBuf>,
    /// The engine script, when it has one (runs in the background).
    pub main: Option<PathBuf>,
    pub icon: String,
    pub permissions: Vec<String>,
    pub enabled: bool,
    pub folder: PathBuf,
    /// Why it can't be used, if it can't.
    pub problem: Option<String>,
    /// Its page is part of Pious (a feature it provides), not a file.
    pub builtin: bool,
    /// The built-in feature it turns on, if any.
    pub provides: Option<String>,
    /// What it brings, in words ("Death sound", "2 themes"…).
    pub brings: Vec<String>,
    /// Roblox client files: path in a version folder → file in the plugin.
    #[serde(skip)]
    pub client_files: BTreeMap<String, PathBuf>,
    /// Theme folders (each with a theme.json).
    pub themes: Vec<PathBuf>,
    pub css: Option<PathBuf>,
    pub icons: BTreeMap<String, PathBuf>,
    pub sounds: BTreeMap<String, PathBuf>,
}

/// The Macros plugin's manifest, as shipped in `plugins/macros`. Written
/// back for portable copies that had Macros on but no plugins folder.
const MACROS_MANIFEST: &str = include_str!("../../../plugins/macros/plugin.json");

/// A copy whose plugins folder has no Macros plugin (a lone pious.exe, or
/// an older install) gets it written once, so Macros never just vanishes
/// on update. Only once: deleting the folder afterwards removes Macros, as
/// it should.
pub fn ensure_macros_plugin() {
    let marker = store::data_dir().join(".macros-plugin-placed");
    if marker.exists() {
        return;
    }
    let present = list(&BTreeSet::new()).iter().any(|p| p.provides.as_deref() == Some("macros"));
    if !present {
        let folder = dir().join("macros");
        if std::fs::create_dir_all(&folder).is_err() || std::fs::write(folder.join("plugin.json"), MACROS_MANIFEST).is_err() {
            // Try again next start.
            return;
        }
        let _ = list(&BTreeSet::new());
    }
    let _ = std::fs::write(marker, b"");
}

/// Macros used to be kept in Pious's own settings. They move into the
/// Macros plugin's data (`data/macros.json`), which its engine owns, the
/// first time a plugin providing macros is there. Returns whether Pious's
/// copy can be cleared (it's in the plugin now, or there was nothing).
pub fn migrate_legacy_macros(macros: &serde_json::Value, settings: &serde_json::Value, autoclicker: &serde_json::Value) -> bool {
    let empty = |v: &serde_json::Value| v.is_null() || v.as_array().is_some_and(Vec::is_empty);
    if empty(macros) && settings.is_null() && autoclicker.is_null() {
        return true;
    }
    let Some(plugin) = list(&BTreeSet::new()).into_iter().find(|p| p.provides.as_deref() == Some("macros")) else {
        return false;
    };
    let file = plugin.folder.join("data").join("macros.json");
    if file.is_file() {
        // Already there (an earlier start moved them): never overwrite.
        return true;
    }
    let mut data = serde_json::json!({ "macros": if macros.is_null() { serde_json::json!([]) } else { macros.clone() } });
    if !settings.is_null() {
        data["settings"] = settings.clone();
    }
    if !autoclicker.is_null() {
        data["autoclicker"] = autoclicker.clone();
    }
    let written = std::fs::create_dir_all(plugin.folder.join("data")).is_ok()
        && serde_json::to_vec_pretty(&data).is_ok_and(|json| {
            let partial = file.with_extension("part");
            std::fs::write(&partial, json).is_ok() && std::fs::rename(&partial, &file).is_ok()
        });
    written
}

/// Whether a working Macros plugin was found the last time plugins were
/// read.
pub fn macros_present() -> bool {
    MACROS_PRESENT.load(Ordering::Relaxed)
}

/// Notes the first problem, and passes on what worked.
fn keep(problem: &mut Option<String>, result: Result<PathBuf, String>) -> Option<PathBuf> {
    match result {
        Ok(path) => Some(path),
        Err(error) => {
            problem.get_or_insert(error);
            None
        }
    }
}

/// A file inside a plugin's folder, or why not.
fn inside(folder: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = folder.join(relative);
    let ok = path.canonicalize().ok().zip(folder.canonicalize().ok()).is_some_and(|(p, f)| p.starts_with(f));
    if ok { Ok(path) } else { Err(format!("{relative} is missing from the plugin's folder.")) }
}

/// Every file under `dir`, relative, with `/`.
fn walk(dir: &Path) -> Vec<String> {
    fn visit(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, out);
            } else if let Ok(relative) = path.strip_prefix(root) {
                out.push(relative.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"));
            }
        }
    }
    let mut out = Vec::new();
    visit(dir, dir, &mut out);
    out
}

/// What each permission lets a plugin do, in words.
pub const PERMISSIONS: [(&str, &str); 10] = [
    ("read", "See your games, accounts, friends and running games"),
    ("launch", "Start and join games"),
    ("notify", "Show notices"),
    ("navigate", "Open Pious pages"),
    ("macros", "Run your macros"),
    ("links", "Open web links"),
    ("input", "Press keys, click and read the screen, in Roblox and other windows"),
    ("hotkeys", "Use keyboard shortcuts"),
    ("run", "Open programs, files and web links"),
    ("full", "Full access to Pious: change any setting and do anything Pious can"),
];

/// `plugins` (from [`list`]) with whether each is turned on now.
pub fn with_enabled(plugins: &[Plugin], enabled: &BTreeSet<String>) -> Vec<Plugin> {
    plugins.iter().map(|p| Plugin { enabled: enabled.contains(&p.id) && p.problem.is_none(), ..p.clone() }).collect()
}

/// Reads one plugin folder. Never fails: a broken plugin comes back with
/// its `problem`, and nothing from it is used.
fn read_plugin(folder: PathBuf, enabled: &BTreeSet<String>) -> Plugin {
    let fallback = folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let parsed = std::fs::read(folder.join("plugin.json"))
        .map_err(|_| "It has no plugin.json.".to_owned())
        .and_then(|bytes| serde_json::from_slice::<Manifest>(&bytes).map_err(|e| format!("Its plugin.json doesn't read ({e}).")));
    let (manifest, mut problem) = match parsed {
        Ok(m) => (m, None),
        Err(problem) => (Manifest::default(), Some(problem)),
    };
    let id = if manifest.id.trim().is_empty() { fallback.clone() } else { manifest.id.trim().to_owned() };
    // Pages and every other file must stay inside the plugin's own folder.
    let page = manifest.page.as_deref().and_then(|p| keep(&mut problem, inside(&folder, p).map_err(|_| "Its page is missing.".into())));
    let main = manifest.main.as_deref().and_then(|p| keep(&mut problem, inside(&folder, p).map_err(|_| "Its engine script is missing.".into())));
    let overlay = manifest.overlay.as_deref().and_then(|p| keep(&mut problem, inside(&folder, p).map_err(|_| "Its overlay panel is missing.".into())));
    let provides = manifest.provides.filter(|f| FEATURES.contains(&f.as_str()));
    let mut brings = Vec::new();

    // Roblox client files.
    let mut client_files = BTreeMap::new();
    if let Some(client) = manifest.client.as_deref() {
        if let Some(root) = keep(&mut problem, inside(&folder, client)) {
            for relative in walk(&root) {
                client_files.insert(relative.clone(), root.join(&relative));
            }
        }
    }
    let shortcuts = [
        ("content/sounds/ouch.ogg", manifest.death_sound.as_deref(), "Death sound"),
        ("content/textures/Cursors/KeyboardMouse/ArrowCursor.png", manifest.cursor.as_deref(), "Cursor"),
        ("content/textures/Cursors/KeyboardMouse/ArrowFarCursor.png", manifest.cursor_far.as_deref(), ""),
        (crate::core::crosshair::TARGET, manifest.shiftlock.as_deref(), "Shift lock cursor"),
    ];
    for (target, file, label) in shortcuts {
        if let Some(path) = file.and_then(|f| keep(&mut problem, inside(&folder, f))) {
            client_files.insert(target.to_owned(), path);
            if !label.is_empty() {
                brings.push(label.to_owned());
            }
        }
    }
    for (target, file) in &manifest.replace {
        if !crate::core::tweaks::safe_relative(target) {
            problem.get_or_insert(format!("{target} isn't a path inside Roblox's folder."));
            continue;
        }
        if let Some(path) = keep(&mut problem, inside(&folder, file)) {
            client_files.insert(target.clone(), path);
        }
    }
    let other_files = client_files.len().saturating_sub(brings.len() + usize::from(client_files.contains_key("content/textures/Cursors/KeyboardMouse/ArrowFarCursor.png")));
    if other_files > 0 {
        brings.push(if other_files == 1 { "1 Roblox file".into() } else { format!("{other_files} Roblox files") });
    }

    let themes: Vec<PathBuf> = manifest.themes.iter().filter_map(|t| keep(&mut problem, inside(&folder, t))).collect();
    if !themes.is_empty() {
        brings.push(if themes.len() == 1 { "A theme".into() } else { format!("{} themes", themes.len()) });
    }
    let css = manifest.css.as_deref().and_then(|c| keep(&mut problem, inside(&folder, c)));
    if css.is_some() {
        brings.push("Interface style".into());
    }
    let icons: BTreeMap<String, PathBuf> = manifest.icons.iter().filter_map(|(name, f)| Some((name.clone(), keep(&mut problem, inside(&folder, f))?))).collect();
    if !icons.is_empty() {
        brings.push(format!("{} icon{}", icons.len(), if icons.len() == 1 { "" } else { "s" }));
    }
    let sounds: BTreeMap<String, PathBuf> = manifest.sounds.iter().filter_map(|(name, f)| Some((name.clone(), keep(&mut problem, inside(&folder, f))?))).collect();
    if !sounds.is_empty() {
        brings.push("Sounds".into());
    }
    if provides.as_deref() == Some("macros") {
        brings.insert(0, "Macros and the auto-clicker".into());
    }

    let mut permissions: Vec<String> = manifest.permissions.into_iter().filter(|p| PERMISSIONS.iter().any(|(k, _)| k == p)).collect();
    permissions.dedup();
    Plugin {
        enabled: enabled.contains(&id) && problem.is_none(),
        name: if manifest.name.trim().is_empty() { fallback } else { manifest.name },
        id,
        version: manifest.version,
        author: manifest.author,
        description: manifest.description,
        page,
        overlay,
        main,
        icon: manifest.icon.unwrap_or_else(|| "puzzle".into()),
        permissions,
        folder,
        problem,
        builtin: provides.is_some(),
        provides,
        brings,
        client_files,
        themes,
        css,
        icons,
        sounds,
    }
}

pub fn list(enabled: &BTreeSet<String>) -> Vec<Plugin> {
    let mut plugins: Vec<Plugin> = Vec::new();
    for dir in dirs() {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.flatten().filter(|e| e.path().is_dir()) {
            let plugin = read_plugin(entry.path(), enabled);
            // The first folder wins (the user's over the repository's).
            if !plugins.iter().any(|p| p.id == plugin.id) {
                plugins.push(plugin);
            }
        }
    }
    MACROS_PRESENT.store(plugins.iter().any(|p| p.provides.as_deref() == Some("macros") && p.problem.is_none()), Ordering::Relaxed);
    plugins.sort_by(|a, b| (!a.builtin, a.name.to_lowercase()).cmp(&(!b.builtin, b.name.to_lowercase())));
    plugins
}

/// The Roblox client files of every enabled plugin, read: (path in a
/// version folder, contents). Later plugins (by name) win. A file that
/// can't be read is skipped.
pub fn client_files(enabled: &BTreeSet<String>) -> Vec<(String, Vec<u8>)> {
    list(enabled)
        .into_iter()
        .filter(|p| p.enabled)
        .flat_map(|p| p.client_files.into_iter())
        .filter_map(|(target, path)| Some((target, std::fs::read(path).ok()?)))
        .collect()
}

/// The interface extras of enabled plugins (and the theme in use), for the
/// window: stylesheets, icons and sounds.
#[derive(Debug, Clone, Default, Serialize)]
pub struct UiAssets {
    /// Stylesheets, in order (plugins', then the theme's, which wins).
    pub css: Vec<String>,
    /// Icon name → file (shown through the asset protocol).
    pub icons: BTreeMap<String, PathBuf>,
    pub sounds: BTreeMap<String, PathBuf>,
    /// A font file the theme brings, and its family name.
    pub font: Option<(String, PathBuf)>,
}

/// Reads the stylesheets and lists icons and sounds of enabled plugins and
/// the theme in use. Never fails: anything unreadable is left out.
pub fn ui_assets(enabled: &BTreeSet<String>, theme: Option<&str>) -> UiAssets {
    let mut out = UiAssets::default();
    let plugins: Vec<Plugin> = list(enabled).into_iter().filter(|p| p.enabled).collect();
    for p in &plugins {
        if let Some(css) = p.css.as_ref().and_then(|c| std::fs::read_to_string(c).ok()) {
            out.css.push(css);
        }
        out.icons.extend(p.icons.clone());
        out.sounds.extend(p.sounds.clone());
    }
    if let Some(theme) = theme.and_then(|id| super::themes::find(id, &plugins)) {
        if let Some(css) = theme.css.as_ref().and_then(|c| std::fs::read_to_string(c).ok()) {
            out.css.push(css);
        }
        out.icons.extend(theme.icons.clone());
        out.sounds.extend(theme.sounds.clone());
        out.font = theme.font_file.clone().map(|f| (theme.font.clone().unwrap_or_else(|| theme.name.clone()), f));
    }
    out
}

const SDK: &str = r#"// The Pious plugin API (Pious keeps this file current; don't edit it).
// Load it first, in a plugin's page or engine:
//
//   <script src="pious-plugin.js"></script>
//
// then use the global `pious`. Every method is in docs/PLUGIN-API.md
// (Help → Plugin API in Pious):
//
//   const snap = await pious.call("snapshot");          // needs "read"
//   await pious.input.key("KeyE", true);                // needs "input"
//   pious.on("hotkey", ({ id, pressed }) => { ... });   // needs "hotkeys"
//   pious.handle("run", async (args) => "Done");        // answer Pious's requests
//   await pious.sleep(250);                             // accurate, even in the background
//
// The page gets Pious's colors as CSS variables (--bg, --text, --accent,
// --surface… as "r g b") and the class "pious". To look like Pious, link
// its stylesheet (also kept current by Pious) and use its classes, listed
// in docs/PLUGIN-UI.md (Help → Plugin UI):
//
//   <link rel="stylesheet" href="pious-ui.css" />
//   el.innerHTML = await pious.ui.icon("play");         // Pious's icons

(() => {
const waiting = new Map();
const listeners = new Map();
const handlers = new Map();
let next = 0;

function emit(event, value) {
  for (const listener of listeners.get(event) ?? []) {
    try {
      listener(value);
    } catch (e) {
      console.error(e);
    }
  }
}

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
    if (message.event === "request") answer(message.value);
    else emit(message.event, message.value);
  }
});

async function answer({ rid, method, args }) {
  const handler = handlers.get(method);
  try {
    if (!handler) throw new Error("This plugin doesn't do " + method + ".");
    const value = await handler(args ?? {});
    await pious.call("respond", rid, value === undefined ? null : value, null);
  } catch (e) {
    await pious.call("respond", rid, null, String(e?.message ?? e)).catch(() => {});
  }
}

const call = (method, ...args) => {
  const id = ++next;
  return new Promise((resolve, reject) => {
    waiting.set(id, { resolve, reject });
    window.parent.postMessage({ pious: "call", id, method, args }, "*");
  });
};

window.pious = {
  call,
  on(event, listener) {
    if (!listeners.has(event)) listeners.set(event, new Set());
    listeners.get(event).add(listener);
    window.parent.postMessage({ pious: "subscribe", event }, "*");
    return () => listeners.get(event)?.delete(listener);
  },
  /** Answers Pious's requests for `method` (see docs/PLUGIN-API.md). */
  handle(method, fn) {
    handlers.set(method, fn);
  },
  sleep: (ms) => call("sleep", ms),
  send: (message) => call("send", message),
  status: (info) => call("status", info),
  storage: {
    read: (name) => call("storage.read", name),
    write: (name, text) => call("storage.write", name, text),
    list: () => call("storage.list"),
    remove: (name) => call("storage.remove", name),
  },
  input: {
    key: (code, down, target = null) => call("input.key", code, down, target),
    text: (text, target = null) => call("input.text", text, target),
    button: (button, down, target = null, at = null) => call("input.button", button, down, target, at),
    move: (x, y, relative = false, target = null) => call("input.move", x, y, relative, target),
    scroll: (notches, horizontal = false, target = null, at = null) => call("input.scroll", notches, horizontal, target, at),
    cursor: () => call("input.cursor"),
    pixel: (x, y, target = null) => call("input.pixel", x, y, target),
    windowPoint: (window, fx, fy) => call("input.windowPoint", window, fx, fy),
    watch: (on) => call("input.watch", on),
    recordStart: () => call("input.recordStart"),
    recordStop: () => call("input.recordStop"),
  },
  windows: {
    roblox: () => call("windows.roblox"),
    focus: (window) => call("windows.focus", window),
    foreground: () => call("windows.foreground"),
  },
  hotkeys: { set: (list) => call("hotkeys.set", list) },
  settings: { get: () => call("settings.get"), set: (patch) => call("settings.set", patch) },
  app: { invoke: (command, args = {}) => call("app.invoke", command, args) },
  ui: {
    css: (text) => call("ui.css", text),
    /** This plugin's panel in the in-game overlay ("overlay" in plugin.json). */
    overlay: { shown: () => call("overlay.shown"), show: (on) => call("overlay.show", on) },
    /** Pious's icons as SVG markup, by name: one, or a map of several. */
    icon: async (name) => (await call("ui.icons", [name]))[name] ?? "",
    icons: (names) => call("ui.icons", names),
  },
};

window.parent.postMessage({ pious: "ready" }, "*");

// How tall the page's content is, so the in-game overlay can fit a plugin's
// panel to it.
let lastHeight = 0;
let sizing = 0;
function reportSize() {
  sizing = 0;
  if (!document.body) return;
  let bottom = 0;
  for (const el of document.body.children) {
    if (el.tagName === "SCRIPT") continue;
    bottom = Math.max(bottom, el.getBoundingClientRect().bottom + window.scrollY);
  }
  const height = Math.ceil(bottom);
  if (height !== lastHeight) {
    lastHeight = height;
    window.parent.postMessage({ pious: "size", height }, "*");
  }
}
function watchSize() {
  const later = () => sizing || (sizing = requestAnimationFrame(reportSize));
  new ResizeObserver(later).observe(document.body);
  new MutationObserver(later).observe(document.body, { childList: true, subtree: true, attributes: true, characterData: true });
  later();
}
if (document.body) watchSize();
else document.addEventListener("DOMContentLoaded", watchSize);
})();
"#;

/// Pious's design system for plugin pages (docs/PLUGIN-UI.md): app.css, then
/// its components as plain classes.
const APP_CSS: &str = include_str!("../../../src/app.css");
const KIT_CSS: &str = include_str!("../../../src/plugin-ui/kit.css");
const FONTS: [(&str, &[u8]); 5] = [
    ("Manrope-Regular.ttf", include_bytes!("../../../assets/fonts/Manrope-Regular.ttf")),
    ("Manrope-Medium.ttf", include_bytes!("../../../assets/fonts/Manrope-Medium.ttf")),
    ("Manrope-SemiBold.ttf", include_bytes!("../../../assets/fonts/Manrope-SemiBold.ttf")),
    ("Manrope-Bold.ttf", include_bytes!("../../../assets/fonts/Manrope-Bold.ttf")),
    ("Manrope-ExtraBold.ttf", include_bytes!("../../../assets/fonts/Manrope-ExtraBold.ttf")),
];

/// `pious-ui.css`. The fonts are inside it: a plugin page is walled off
/// (no origin), and fonts from anywhere else would need permission headers.
fn ui_css() -> String {
    use base64::Engine;
    let mut css = APP_CSS.to_owned();
    for (name, bytes) in FONTS {
        let data = format!("data:font/ttf;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes));
        css = css.replace(&format!("../assets/fonts/{name}"), &data);
    }
    format!("/* Pious's look for plugin pages (Pious keeps this file current; don't edit it). See docs/PLUGIN-UI.md. */\n{css}\n{KIT_CSS}")
}

static UI_CSS: std::sync::LazyLock<String> = std::sync::LazyLock::new(ui_css);

/// The page a plugin's engine runs in (Pious writes it next to the engine).
fn engine_html(main: &str) -> String {
    format!("<!doctype html>\n<meta charset=\"utf-8\">\n<!-- Written by Pious: runs this plugin's engine in the background. -->\n<script src=\"pious-plugin.js\"></script>\n<script src=\"{main}\"></script>\n")
}

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
    <!-- Pious's look: its stylesheet and classes (docs/PLUGIN-UI.md). -->
    <link rel="stylesheet" href="pious-ui.css" />
  </head>
  <body>
    <div class="page">
      <div class="page-header">
        <div class="col grow" style="gap: 2px">
          <h1 class="page-title">Example plugin</h1>
          <span class="meta">Edit index.html in this plugin's folder to make it yours.</span>
        </div>
        <button class="btn primary" id="hello">Say hello</button>
      </div>
      <section class="group">
        <span class="label">Right now</span>
        <div class="glass list" id="out"></div>
      </section>
    </div>
    <script src="pious-plugin.js"></script>
    <script>
      const out = document.getElementById("out");
      const row = (title, description) =>
        `<div class="setting-row"><div class="text"><span class="title">${title}</span><span class="description">${description}</span></div></div>`;
      const draw = (snap) => {
        const playing = snap.friends.filter((f) => f.status === "in_game");
        out.innerHTML =
          row(`${snap.running.length} game(s) running`, "Started from Pious") +
          '<hr class="divider" />' +
          row(`${playing.length} friend(s) playing`, playing.map((f) => f.display_name).join(", ") || "Nobody right now");
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
    std::fs::write(folder.join("pious-ui.css"), UI_CSS.as_bytes()).map_err(|e| e.to_string())?;
    Ok(folder)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A plugin with nothing set, for tests.
    pub(crate) fn blank() -> Plugin {
        read_plugin(std::env::temp_dir().join("pious-no-such-plugin"), &BTreeSet::new())
    }

    fn scratch() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pious-plugin-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_macros_plugin_ships_as_a_folder() {
        let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("plugins").join("macros");
        let plugin = read_plugin(folder, &[crate::core::model::MACROS_PLUGIN.to_owned()].into());
        assert!(plugin.problem.is_none(), "{:?}", plugin.problem);
        assert_eq!(plugin.id, crate::core::model::MACROS_PLUGIN);
        assert_eq!(plugin.provides.as_deref(), Some("macros"));
        assert!(plugin.enabled && plugin.builtin);
        assert!(plugin.overlay.as_ref().is_some_and(|o| o.ends_with("overlay.html")), "its in-game overlay panel");
        // The copy written for portable installs is the same file.
        assert!(serde_json::from_str::<Manifest>(MACROS_MANIFEST).is_ok());
    }

    #[test]
    fn reads_assets_and_keeps_them_inside() {
        let dir = scratch();
        std::fs::write(dir.join("oof.ogg"), b"oof").unwrap();
        std::fs::write(dir.join("style.css"), "a{}").unwrap();
        std::fs::create_dir_all(dir.join("client/content/textures")).unwrap();
        std::fs::write(dir.join("client/content/textures/x.png"), b"png").unwrap();
        std::fs::write(
            dir.join("plugin.json"),
            r#"{ "id": "pack", "death_sound": "oof.ogg", "client": "client", "css": "style.css", "replace": { "content/sounds/hit.ogg": "oof.ogg" } }"#,
        )
        .unwrap();
        let plugin = read_plugin(dir.clone(), &["pack".to_owned()].into());
        assert!(plugin.problem.is_none(), "{:?}", plugin.problem);
        assert!(plugin.client_files.contains_key("content/sounds/ouch.ogg"));
        assert!(plugin.client_files.contains_key("content/textures/x.png"));
        assert!(plugin.client_files.contains_key("content/sounds/hit.ogg"));
        assert!(plugin.brings.iter().any(|b| b == "Death sound"));
        assert!(plugin.css.is_some());

        // Escaping the plugin's folder or Roblox's makes it unusable.
        std::fs::write(dir.join("plugin.json"), r#"{ "id": "pack", "death_sound": "../../x.ogg" }"#).unwrap();
        assert!(read_plugin(dir.clone(), &["pack".to_owned()].into()).problem.is_some());
        std::fs::write(dir.join("plugin.json"), r#"{ "id": "pack", "replace": { "../evil.dll": "oof.ogg" } }"#).unwrap();
        let bad = read_plugin(dir.clone(), &["pack".to_owned()].into());
        assert!(bad.problem.is_some() && !bad.enabled);

        // An engine script must be inside too.
        std::fs::write(dir.join("plugin.json"), r#"{ "id": "pack", "main": "../engine.js" }"#).unwrap();
        assert!(read_plugin(dir.clone(), &["pack".to_owned()].into()).problem.is_some());

        // A broken manifest is listed, not used.
        std::fs::write(dir.join("plugin.json"), "{ nope").unwrap();
        let broken = read_plugin(dir.clone(), &["pack".to_owned()].into());
        assert!(broken.problem.is_some() && !broken.enabled && broken.client_files.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Keeps every plugin's copy of the API file current (and writes it for
/// plugins with a page or an engine that don't have it yet), and the page
/// each engine runs in.
pub fn refresh_sdk() {
    for plugin in list(&BTreeSet::new()) {
        if plugin.page.is_none() && plugin.main.is_none() && plugin.overlay.is_none() {
            continue;
        }
        let path = plugin.folder.join("pious-plugin.js");
        if std::fs::read_to_string(&path).ok().as_deref() != Some(SDK) {
            let _ = std::fs::write(path, SDK);
        }
        if plugin.page.is_some() || plugin.overlay.is_some() {
            let path = plugin.folder.join("pious-ui.css");
            // Its size first: reading it back is only needed when they match.
            let same = std::fs::metadata(&path).is_ok_and(|m| m.len() as usize == UI_CSS.len())
                && std::fs::read_to_string(&path).ok().as_deref() == Some(UI_CSS.as_str());
            if !same {
                let _ = std::fs::write(path, UI_CSS.as_bytes());
            }
        }
        if let (Some(main), Some(page)) = (&plugin.main, super::pluginapi::engine_page(&plugin)) {
            let relative = main.strip_prefix(&plugin.folder).map(|r| r.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join("/")).unwrap_or_default();
            let html = engine_html(&relative);
            if std::fs::read_to_string(&page).ok().as_deref() != Some(html.as_str()) {
                let _ = std::fs::write(page, html);
            }
        }
    }
}
