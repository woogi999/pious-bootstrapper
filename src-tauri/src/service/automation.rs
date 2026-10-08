//! Macros and the auto-clicker: running them, stopping them, and recording
//! new macros from the keyboard and mouse.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use super::{Service, Shared, Tone};
use crate::core::automation::{self, ClickMode, InputTarget, Macro, Repeat, Sink};
use crate::core::{process, stats};

/// What's running right now.
#[derive(Default)]
pub struct Automation {
    /// Running macros and the flag that stops each.
    pub running: HashMap<Uuid, Arc<AtomicBool>>,
    pub clicker: Option<Arc<AtomicBool>>,
    /// A macro recording, since when.
    pub recording: Option<Instant>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AutomationStatus {
    pub running: Vec<Uuid>,
    pub clicking: bool,
    pub recording: bool,
}

pub fn status_of(a: &Automation) -> AutomationStatus {
    let mut running: Vec<Uuid> = a.running.keys().copied().collect();
    running.sort();
    AutomationStatus { running, clicking: a.clicker.is_some(), recording: a.recording.is_some() }
}

/// Lets playback bring a game forward.
struct Games;

impl automation::Host for Games {
    fn focus_roblox(&self) {
        if let Some(pid) = process::roblox_pids().into_iter().next() {
            process::focus(pid);
        }
    }
}

impl Service {
    /// The built-in Macros plugin is on (it starts off).
    pub fn macros_enabled(&self) -> bool {
        self.read().bootstrapper.preferences.plugins.contains(crate::core::model::MACROS_PLUGIN)
    }

    fn require_macros(&self) -> Result<(), String> {
        if self.macros_enabled() {
            Ok(())
        } else {
            Err("Macros are a plugin that's off. Turn it on in Settings → Plugins.".into())
        }
    }

    /// The windows input goes to.
    fn sink_for(&self, target: InputTarget, accounts: &[Uuid]) -> Result<Sink, String> {
        if target == InputTarget::System {
            return Ok(Sink::System);
        }
        if target == InputTarget::Accounts {
            let pids: Vec<u32> = {
                let s = self.read();
                s.instances.iter().filter(|i| i.account.is_some_and(|a| accounts.contains(&a))).filter_map(|i| i.pid).collect()
            };
            let windows: Vec<isize> = pids.into_iter().filter_map(process::window_of).collect();
            if windows.is_empty() {
                return Err(if accounts.is_empty() {
                    "Pick which accounts' Roblox windows this goes to.".into()
                } else {
                    "None of the picked accounts has a Roblox window open.".into()
                });
            }
            return Ok(Sink::Windows(windows));
        }
        let last = self.read().last_roblox_window;
        let windows: Vec<isize> = process::roblox_pids().into_iter().filter_map(process::window_of).collect();
        if windows.is_empty() {
            return Err("No Roblox window is open to send this to.".into());
        }
        Ok(Sink::Windows(match target {
            InputTarget::AllRoblox => windows,
            // The Roblox window you used last, else any.
            _ => vec![windows.iter().copied().find(|w| Some(*w) == last).unwrap_or(windows[0])],
        }))
    }

    fn automation_busy(&self) -> bool {
        let s = self.read();
        !s.automation.running.is_empty() || s.automation.clicker.is_some() || s.automation.recording.is_some()
    }

    /// Starts a macro, or stops it if it's running.
    pub fn run_macro(self: &Shared, id: Uuid) -> Result<(), String> {
        if let Some(stop) = self.read().automation.running.get(&id) {
            stop.store(true, Ordering::Relaxed);
            return Ok(());
        }
        let found: Option<Macro> = self.read().bootstrapper.preferences.macros.iter().find(|m| m.id == id).cloned();
        let Some(m) = found else { return Err("That macro isn't there anymore.".into()) };
        self.require_macros()?;
        if m.steps.is_empty() {
            return Err(format!("{} has no steps yet.", m.name));
        }
        let sink = self.sink_for(m.target, &m.accounts)?;
        let stop = Arc::new(AtomicBool::new(false));
        let was_busy = self.automation_busy();
        self.mutate(|s| s.automation.running.insert(id, stop.clone()));
        if !was_busy {
            self.apply_hotkeys(false);
        }
        stats::record("macro", json!({ "name": m.name }));
        let me = self.clone();
        std::thread::Builder::new()
            .name("macro".into())
            .spawn(move || {
                let mut round = 0u32;
                loop {
                    round += 1;
                    if !automation::play(&m.steps, m.speed, &stop, &Games, &sink) {
                        break;
                    }
                    let more = match m.repeat {
                        Repeat::Once => false,
                        Repeat::Times => round < m.times.max(1),
                        Repeat::UntilStopped | Repeat::WhileHeld => true,
                    };
                    if !more || stop.load(Ordering::Relaxed) {
                        break;
                    }
                }
                me.mutate(|s| s.automation.running.remove(&id));
                if !me.automation_busy() {
                    me.apply_hotkeys(false);
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn stop_macro(&self, id: Uuid) {
        if let Some(stop) = self.read().automation.running.get(&id) {
            stop.store(true, Ordering::Relaxed);
        }
    }

    /// Stops every macro and the auto-clicker (and a macro recording).
    pub fn stop_automation(self: &Shared) {
        {
            let s = self.read();
            for stop in s.automation.running.values() {
                stop.store(true, Ordering::Relaxed);
            }
            if let Some(stop) = &s.automation.clicker {
                stop.store(true, Ordering::Relaxed);
            }
        }
        if self.read().automation.recording.is_some() {
            let _ = self.finish_macro_recording();
        }
    }

    /// Starts clicking, or stops if it already is.
    pub fn toggle_autoclicker(self: &Shared) {
        if let Some(stop) = self.read().automation.clicker.clone() {
            stop.store(true, Ordering::Relaxed);
            return;
        }
        self.start_autoclicker();
    }

    pub fn start_autoclicker(self: &Shared) {
        if self.read().automation.clicker.is_some() {
            return;
        }
        let settings = self.read().bootstrapper.preferences.autoclicker.clone();
        let sink = match self.require_macros().and_then(|()| self.sink_for(settings.target, &settings.accounts)) {
            Ok(sink) => sink,
            Err(error) => {
                self.toast(Tone::Caution, error);
                return;
            }
        };
        let stop = Arc::new(AtomicBool::new(false));
        let was_busy = self.automation_busy();
        // The modes that follow your mouse need the input hook.
        self.mutate(|s| {
            s.automation.clicker = Some(stop.clone());
            s.input_dirty = true;
        });
        if !was_busy {
            self.apply_hotkeys(false);
        }
        if matches!(settings.mode, ClickMode::MouseHeld | ClickMode::OnClick) {
            let what = if settings.mode == ClickMode::MouseHeld { "Hold the mouse to click" } else { "Your clicks get extra clicks" };
            self.hud("autoclick", &format!("Auto-clicker on · {what}"));
        }
        let me = self.clone();
        let _ = std::thread::Builder::new().name("autoclicker".into()).spawn(move || {
            let game_only = settings.game_only;
            let gate = me.clone();
            let allowed = move || !game_only || gate.read().foreground_roblox.is_some();
            let clicks = automation::autoclick(&settings, &stop, &sink, &allowed);
            stats::record("autoclick", json!({ "clicks": clicks }));
            me.mutate(|s| {
                s.automation.clicker = None;
                s.input_dirty = true;
            });
            if !me.automation_busy() {
                me.apply_hotkeys(false);
            }
        });
    }

    pub fn stop_autoclicker(&self) {
        if let Some(stop) = &self.read().automation.clicker {
            stop.store(true, Ordering::Relaxed);
        }
    }

    /// A hotkey was let go: stops what runs only while it's held.
    pub(super) fn hotkey_released(&self, action: super::hotkeys::Action) {
        use super::hotkeys::Action;
        match action {
            Action::Macro(id) => {
                let held = self.read().bootstrapper.preferences.macros.iter().any(|m| m.id == id && m.repeat == Repeat::WhileHeld);
                if held {
                    self.stop_macro(id);
                }
            }
            Action::Autoclick => {
                if self.read().bootstrapper.preferences.autoclicker.mode == ClickMode::Hold {
                    self.stop_autoclicker();
                }
            }
            _ => {}
        }
    }

    /// Records a new macro, or saves the one being recorded.
    pub fn toggle_macro_recording(self: &Shared) {
        if self.read().automation.recording.is_some() {
            match self.finish_macro_recording() {
                Ok(name) => {
                    self.hud("macro", &format!("Saved {name}"));
                    self.toast(Tone::Positive, format!("Saved {name}. Find it in Macros."));
                }
                Err(error) => self.toast(Tone::Caution, error),
            }
            return;
        }
        if let Err(error) = self.require_macros().and_then(|()| automation::start_recording()) {
            self.toast(Tone::Negative, error);
            return;
        }
        self.mutate(|s| s.automation.recording = Some(Instant::now()));
        self.apply_hotkeys(false);
        let key = crate::service::keys_label(&self.read().bootstrapper.preferences.macro_settings.record_hotkey);
        self.hud("macro", &format!("Recording a macro · {key} to stop"));
    }

    /// Stops recording and saves the macro. Returns its name.
    pub fn finish_macro_recording(self: &Shared) -> Result<String, String> {
        let events = automation::stop_recording();
        self.mutate(|s| s.automation.recording = None);
        self.apply_hotkeys(false);
        let settings = self.read().bootstrapper.preferences.macro_settings.clone();
        // Leave out the keys of the record hotkey itself.
        let ignore: Vec<u16> = settings.record_hotkey.split('+').filter_map(automation::vk_of).collect();
        let steps = automation::to_steps(&events, &ignore, settings.record_moves, settings.record_timing);
        if steps.is_empty() {
            return Err("Nothing was recorded.".into());
        }
        let name = self.mutate(|s| {
            let n = s.bootstrapper.preferences.macros.len() + 1;
            let name = format!("Recorded macro {n}");
            s.bootstrapper.preferences.macros.push(Macro { name: name.clone(), steps, ..Macro::default() });
            s.dirty = true;
            name
        });
        Ok(name)
    }
}

/// Where the pointer is and the color under it.
pub fn cursor_info() -> serde_json::Value {
    let (x, y) = automation::cursor();
    json!({ "x": x, "y": y, "color": automation::color_at(x, y) })
}
