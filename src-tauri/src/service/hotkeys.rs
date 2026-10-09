//! Global hotkeys: the overlay, recording and clipping, and plugins' own
//! (the Macros plugin's macros and auto-clicker).
//!
//! A hotkey set to "only in game" is caught by the input hook while a
//! Roblox window (or the overlay) is in front, and kept from the game, so a
//! key like F12 takes over Roblox's own and keeps working normally in every
//! other program. (A Windows hotkey can't: F12 is reserved for debuggers and
//! often taken, and the game would still get the key.) Hotkeys for
//! everywhere are Windows hotkeys.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use super::{Service, Shared, Tone};
use crate::core::process;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Overlay,
    Record,
    Clip,
    ManualClip,
    /// A plugin's hotkey (its place in `plugin_runtime.hotkeys`).
    Plugin(u32),
}

impl Action {
    fn name(self) -> &'static str {
        match self {
            Action::Overlay => "the overlay",
            Action::Record => "recording",
            Action::Clip => "clipping",
            Action::ManualClip => "clipping a chosen length",
            Action::Plugin(_) => "a plugin",
        }
    }
}

impl Service {
    /// The hotkeys the settings ask for: (action, keys, only in game).
    fn wanted_hotkeys(&self) -> Vec<(Action, String, bool)> {
        let s = self.read();
        let p = &s.bootstrapper.preferences;
        let mut out = Vec::new();
        if p.overlay.enabled {
            out.push((Action::Overlay, p.overlay.hotkey.clone(), p.overlay.game_only));
        }
        let r = &p.recorder;
        if r.recording {
            out.push((Action::Record, r.record_hotkey.clone(), r.game_only));
        }
        if r.clips {
            out.push((Action::Clip, r.clip_hotkey.clone(), r.game_only));
            out.push((Action::ManualClip, r.manual_clip_hotkey.clone(), r.game_only));
        }
        // Plugins' own hotkeys (only enabled plugins keep theirs).
        for (i, (_, _, keys, game_only)) in s.plugin_runtime.hotkeys.iter().enumerate() {
            out.push((Action::Plugin(i as u32), keys.clone(), *game_only));
        }
        out.retain(|(_, keys, _)| !keys.trim().is_empty());
        out
    }

    /// Registers the hotkeys that apply right now. With `announce`,
    /// problems (a taken or invalid key) are shown.
    pub fn apply_hotkeys(&self, announce: bool) {
        let problems = self.apply_hotkeys_report();
        if announce {
            for problem in problems {
                self.toast(Tone::Caution, problem);
            }
        }
    }

    /// Registers the hotkeys that apply right now, and says what couldn't be.
    pub fn apply_hotkeys_report(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let wanted = self.wanted_hotkeys();
        let in_game = self.read().in_game;
        let shortcuts = self.app().global_shortcut();
        let _ = shortcuts.unregister_all();
        let _ = in_game;
        let mut registered = std::collections::HashMap::new();
        let mut in_game_keys: Vec<(String, Action)> = Vec::new();
        for (action, keys, game_only) in wanted {
            if game_only {
                // Checked now so mistakes show up when they're made.
                if keys.parse::<Shortcut>().is_err() {
                    problems.push(format!("{keys} isn't a key combination Pious can use for {}.", action.name()));
                } else if in_game_keys.iter().any(|(k, _)| k.eq_ignore_ascii_case(&keys)) {
                    problems.push(format!("{keys} is already used for something else in Pious."));
                } else {
                    in_game_keys.push((keys, action));
                }
                continue;
            }
            let Ok(shortcut) = keys.parse::<Shortcut>() else {
                problems.push(format!("{keys} isn't a key combination Pious can use for {}.", action.name()));
                continue;
            };
            if registered.contains_key(&shortcut.id()) {
                problems.push(format!("{keys} is already used for something else in Pious."));
                continue;
            }
            match shortcuts.register(shortcut) {
                Ok(()) => {
                    registered.insert(shortcut.id(), action);
                }
                Err(error) => {
                    problems.push(format!("Couldn't use {keys} for {}; another app may have it ({error}).", action.name()));
                }
            }
        }
        let mut s = self.read();
        s.hotkeys = registered;
        if s.game_hotkeys != in_game_keys {
            s.game_hotkeys = in_game_keys;
            s.input_dirty = true;
        }
        problems
    }

    fn on_hotkey(self: &Shared, id: u32, pressed: bool) {
        let Some(action) = self.read().hotkeys.get(&id).copied() else { return };
        self.hotkey_action(action, pressed);
    }

    /// An in-game hotkey the input hook caught (its index in `game_hotkeys`).
    pub(super) fn game_hotkey(self: &Shared, index: u32, pressed: bool) {
        let Some(action) = self.read().game_hotkeys.get(index as usize).map(|(_, a)| *a) else { return };
        self.hotkey_action(action, pressed);
    }

    fn hotkey_action(self: &Shared, action: Action, pressed: bool) {
        // A plugin's: the plugin hears about presses and releases alike.
        if let Action::Plugin(i) = action {
            let found = self.read().plugin_runtime.hotkeys.get(i as usize).map(|(p, id, ..)| (p.clone(), id.clone()));
            if let Some((plugin, id)) = found {
                self.plugin_event(&plugin, "engine", "hotkey", serde_json::json!({ "id": id, "pressed": pressed }));
            }
            return;
        }
        // Pious's own hotkeys act when pressed.
        if !pressed {
            return;
        }
        match action {
            Action::Overlay => self.toggle_overlay(),
            Action::Record => self.toggle_recording(),
            Action::Clip => self.save_clip(None, Instant::now()),
            Action::ManualClip => self.ask_clip_length(),
            // Handled above.
            Action::Plugin(_) => {}
        }
    }

    /// Keeps "only in game" hotkeys registered exactly while a game (or
    /// the overlay) is in front.
    pub(super) async fn hotkey_focus_loop(self: Shared) {
        let me = std::process::id();
        let mut roblox: HashSet<u32> = HashSet::new();
        let mut refreshed: Option<Instant> = None;
        loop {
            tokio::time::sleep(Duration::from_millis(250)).await;
            if refreshed.is_none_or(|t| t.elapsed() > Duration::from_secs(2)) {
                roblox = tokio::task::spawn_blocking(process::roblox_pids).await.unwrap_or_default().into_iter().collect();
                refreshed = Some(Instant::now());
            }
            let foreground = process::foreground_window();
            let pid = process::window_pid(foreground);
            let playing = roblox.contains(&pid);
            let in_game = playing || (pid == me && self.read().overlay_open);
            let changed = {
                let mut s = self.read();
                let changed = s.in_game != in_game;
                s.in_game = in_game;
                // Background macros go to the Roblox window used last.
                if playing {
                    s.last_roblox_window = Some(foreground);
                }
                s.foreground_roblox = playing.then_some(pid);
                changed
            };
            if changed {
                self.apply_hotkeys(false);
            }
        }
    }
}

/// The global-shortcut plugin, wired to the service.
pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app: &AppHandle, shortcut, event| {
            if let Some(service) = app.try_state::<Shared>() {
                service.on_hotkey(shortcut.id(), event.state() == ShortcutState::Pressed);
            }
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_hotkeys_parse() {
        for keys in ["Home", "F12", "F8", "Shift+F8", "Alt+Backquote", "Control+Shift+KeyK"] {
            assert!(keys.parse::<Shortcut>().is_ok(), "{keys}");
        }
    }
}
