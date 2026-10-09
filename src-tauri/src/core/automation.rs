//! Input capabilities: playing keyboard and mouse input the way real
//! hardware sends it (or straight to a window in the background), and
//! recording what the user does. The macro engine itself is the Macros
//! plugin (`plugins/macros`); AI apps' input tools use the player here.
//!
//! Keys are named like the browser's `KeyboardEvent.code` ("KeyW",
//! "Space", "ShiftLeft", "F5"), the same names hotkeys use.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Press {
    /// Down, then up.
    #[default]
    Tap,
    Down,
    Up,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Button {
    #[default]
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

fn one() -> u32 {
    1
}

/// One thing a macro does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Step {
    Key {
        key: String,
        #[serde(default)]
        press: Press,
        /// How long a tap holds the key.
        #[serde(default)]
        hold_ms: u32,
    },
    /// Types text as if on a keyboard.
    Text {
        text: String,
        #[serde(default)]
        delay_ms: u32,
    },
    Click {
        #[serde(default)]
        button: Button,
        #[serde(default)]
        press: Press,
        #[serde(default = "one")]
        count: u32,
    },
    /// Moves the pointer to a point on the screen, or by an amount.
    Move {
        x: i32,
        y: i32,
        #[serde(default)]
        relative: bool,
        /// Glides there over this long instead of jumping.
        #[serde(default)]
        duration_ms: u32,
    },
    /// Wheel notches; positive is up (or right).
    Scroll {
        amount: i32,
        #[serde(default)]
        horizontal: bool,
    },
    Wait {
        ms: u32,
        /// Up to this much longer, at random (looks less robotic).
        #[serde(default)]
        random_ms: u32,
    },
    /// The steps inside, `times` times (0 = until stopped).
    Loop { times: u32, steps: Vec<Step> },
    /// Waits until a point on the screen is a color (`#RRGGBB`).
    WaitPixel {
        x: i32,
        y: i32,
        color: String,
        #[serde(default)]
        tolerance: u8,
        /// Gives up after this long (0 = never).
        #[serde(default)]
        timeout_ms: u32,
    },
    /// Brings a Roblox window to the front.
    FocusRoblox,
    /// Opens a program, file or web address.
    Run { target: String },
    /// A note; does nothing.
    Comment { text: String },
}

// ── Playback ─────────────────────────────────────────────────────────────

/// What a playback can ask of the app.
pub trait Host {
    /// Brings a Roblox window forward.
    fn focus_roblox(&self);
}

/// Sleeps, waking early when `stop` is set. Returns false when stopped.
fn pause(ms: f64, stop: &AtomicBool) -> bool {
    let end = Instant::now() + Duration::from_secs_f64(ms.max(0.0) / 1000.0);
    loop {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        let now = Instant::now();
        if now >= end {
            return true;
        }
        std::thread::sleep((end - now).min(Duration::from_millis(8)));
    }
}

fn jitter(ms: u32) -> f64 {
    if ms == 0 { 0.0 } else { crate::core::random::unit() * ms as f64 }
}

/// Where playback sends its input.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub enum Sink {
    /// Like real hardware: whatever window is in front gets it, and the
    /// real pointer moves.
    System,
    /// Posted straight to these windows (by handle), in the background.
    /// The real pointer and keyboard are left alone.
    Windows(Vec<isize>),
}

/// Sends a playback's input to its [`Sink`] and remembers what it holds
/// down, so it can all be let go at the end.
struct Out<'a> {
    sink: &'a Sink,
    /// Where the pointer is for background windows (screen coordinates);
    /// `None` = the middle of each window.
    at: Option<(i32, i32)>,
    keys: Vec<String>,
    buttons: Vec<Button>,
    /// When background windows were last told they're active.
    woken: Option<Instant>,
}

impl<'a> Out<'a> {
    fn new(sink: &'a Sink) -> Self {
        Self { sink, at: None, keys: Vec::new(), buttons: Vec::new(), woken: None }
    }

    /// Roblox ignores input while it thinks it's in the background, so
    /// background windows are told they're active now and then.
    fn wake(&mut self) {
        if let Sink::Windows(windows) = self.sink {
            if self.woken.is_none_or(|t| t.elapsed() > Duration::from_millis(400)) {
                self.woken = Some(Instant::now());
                for &w in windows {
                    background::activate(w);
                }
            }
        }
    }

    fn key(&mut self, key: &str, down: bool) {
        if down {
            if !self.keys.iter().any(|k| k == key) {
                self.keys.push(key.to_owned());
            }
        } else {
            self.keys.retain(|k| k != key);
        }
        match self.sink {
            Sink::System => input::key(key, down),
            Sink::Windows(windows) => {
                self.wake();
                for &w in windows {
                    background::key(w, key, down);
                }
            }
        }
    }

    fn char(&mut self, c: char) {
        match self.sink {
            Sink::System => input::text(c.encode_utf8(&mut [0; 4])),
            Sink::Windows(windows) => {
                self.wake();
                for &w in windows {
                    background::char(w, c);
                }
            }
        }
    }

    fn button(&mut self, button: Button, down: bool) {
        if down {
            self.buttons.push(button);
        } else if let Some(i) = self.buttons.iter().position(|b| *b == button) {
            self.buttons.remove(i);
        }
        match self.sink {
            Sink::System => input::button(button, down),
            Sink::Windows(windows) => {
                self.wake();
                for &w in windows {
                    background::button(w, button, down, self.at);
                }
            }
        }
    }

    fn move_to(&mut self, x: i32, y: i32) {
        match self.sink {
            Sink::System => input::move_to(x, y),
            Sink::Windows(windows) => {
                self.at = Some((x, y));
                for &w in windows {
                    background::move_to(w, x, y);
                }
            }
        }
    }

    /// A relative move. In the background this moves the pointer Pious
    /// keeps for the window (games read camera turns from the real mouse,
    /// so those only work with [`Sink::System`]).
    fn move_by(&mut self, dx: i32, dy: i32) {
        match self.sink {
            Sink::System => input::move_by(dx, dy),
            Sink::Windows(_) => {
                let (x, y) = self.cursor();
                self.move_to(x + dx, y + dy);
            }
        }
    }

    fn scroll(&mut self, notches: i32, horizontal: bool) {
        match self.sink {
            Sink::System => input::scroll(notches, horizontal),
            Sink::Windows(windows) => {
                self.wake();
                for &w in windows {
                    background::scroll(w, notches, horizontal, self.at);
                }
            }
        }
    }

    fn cursor(&self) -> (i32, i32) {
        match (self.sink, self.at) {
            (Sink::System, _) => input::cursor(),
            (Sink::Windows(_), Some(at)) => at,
            (Sink::Windows(windows), None) => windows.first().map(|&w| background::center(w)).unwrap_or((0, 0)),
        }
    }

    fn pixel(&self, x: i32, y: i32) -> Option<(u8, u8, u8)> {
        match self.sink {
            Sink::System => input::pixel(x, y),
            // What the window shows, even when it's covered.
            Sink::Windows(windows) => windows.first().and_then(|&w| background::pixel(w, x, y)),
        }
    }

    /// Lets go of everything still held down.
    fn release(&mut self) {
        for key in std::mem::take(&mut self.keys) {
            self.key(&key, false);
        }
        for button in std::mem::take(&mut self.buttons) {
            self.button(button, false);
        }
    }
}

/// Plays steps once. Returns false when stopped part-way. Anything held
/// down is let go before returning.
pub fn play(steps: &[Step], speed: f32, stop: &AtomicBool, host: &dyn Host, sink: &Sink) -> bool {
    let mut out = Out::new(sink);
    let finished = run_steps(steps, 1.0 / speed.clamp(0.1, 20.0) as f64, stop, host, &mut out);
    out.release();
    finished
}

fn run_steps(steps: &[Step], scale: f64, stop: &AtomicBool, host: &dyn Host, out: &mut Out) -> bool {
    for step in steps {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        let ok = match step {
            Step::Key { key, press, hold_ms } => match press {
                Press::Down => {
                    out.key(key, true);
                    true
                }
                Press::Up => {
                    out.key(key, false);
                    true
                }
                Press::Tap => {
                    out.key(key, true);
                    let ok = pause((*hold_ms).max(15) as f64 * scale, stop);
                    out.key(key, false);
                    ok
                }
            },
            Step::Text { text, delay_ms } => {
                let mut ok = true;
                for c in text.chars() {
                    out.char(c);
                    if !pause((*delay_ms).max(4) as f64 * scale, stop) {
                        ok = false;
                        break;
                    }
                }
                ok
            }
            Step::Click { button, press, count } => match press {
                Press::Down => {
                    out.button(*button, true);
                    true
                }
                Press::Up => {
                    out.button(*button, false);
                    true
                }
                Press::Tap => {
                    let mut ok = true;
                    for i in 0..(*count).max(1) {
                        if i > 0 && !pause(40.0 * scale, stop) {
                            ok = false;
                            break;
                        }
                        out.button(*button, true);
                        std::thread::sleep(Duration::from_millis(12));
                        out.button(*button, false);
                    }
                    ok
                }
            },
            Step::Move { x, y, relative, duration_ms } => {
                if *duration_ms == 0 {
                    if *relative { out.move_by(*x, *y) } else { out.move_to(*x, *y) }
                    true
                } else {
                    glide(out, *x, *y, *relative, *duration_ms as f64 * scale, stop)
                }
            }
            Step::Scroll { amount, horizontal } => {
                out.scroll(*amount, *horizontal);
                true
            }
            Step::Wait { ms, random_ms } => pause((*ms as f64 + jitter(*random_ms)) * scale, stop),
            Step::Loop { times, steps } => {
                let mut ok = true;
                let mut round = 0u32;
                while *times == 0 || round < *times {
                    round += 1;
                    if !run_steps(steps, scale, stop, host, out) {
                        ok = false;
                        break;
                    }
                    // An empty or instant loop shouldn't spin the CPU.
                    if steps.is_empty() && !pause(50.0, stop) {
                        ok = false;
                        break;
                    }
                }
                ok
            }
            Step::WaitPixel { x, y, color, tolerance, timeout_ms } => {
                let want = parse_color(color).unwrap_or((0, 0, 0));
                let start = Instant::now();
                loop {
                    if let Some(got) = out.pixel(*x, *y) {
                        let near = |a: u8, b: u8| a.abs_diff(b) <= *tolerance;
                        if near(got.0, want.0) && near(got.1, want.1) && near(got.2, want.2) {
                            break true;
                        }
                    }
                    if *timeout_ms > 0 && start.elapsed() >= Duration::from_millis(*timeout_ms as u64) {
                        break true;
                    }
                    if !pause(25.0, stop) {
                        break false;
                    }
                }
            }
            Step::FocusRoblox => {
                host.focus_roblox();
                pause(80.0, stop)
            }
            Step::Run { target } => {
                let _ = open::that_detached(target.trim());
                true
            }
            Step::Comment { .. } => true,
        };
        if !ok {
            return false;
        }
    }
    true
}

fn glide(out: &mut Out, x: i32, y: i32, relative: bool, ms: f64, stop: &AtomicBool) -> bool {
    let (sx, sy) = out.cursor();
    let (tx, ty) = if relative { (sx + x, sy + y) } else { (x, y) };
    let frames = ((ms / 8.0).ceil() as i32).max(1);
    let (mut lx, mut ly) = (sx, sy);
    for i in 1..=frames {
        let t = i as f64 / frames as f64;
        let e = t * t * (3.0 - 2.0 * t);
        let nx = sx + ((tx - sx) as f64 * e).round() as i32;
        let ny = sy + ((ty - sy) as f64 * e).round() as i32;
        if relative {
            out.move_by(nx - lx, ny - ly);
        } else {
            out.move_to(nx, ny);
        }
        (lx, ly) = (nx, ny);
        if !pause(ms / frames as f64, stop) {
            return false;
        }
    }
    true
}

pub fn parse_color(text: &str) -> Option<(u8, u8, u8)> {
    let hex = text.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(hex, 16).ok()?;
    Some(((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// The color at a point on the screen, as `#RRGGBB`.
pub fn cursor() -> (i32, i32) {
    input::cursor()
}

/// What a window shows (even behind other windows): width, height, RGBA.
pub fn window_picture(window: isize) -> Option<(u32, u32, Vec<u8>)> {
    background::picture(window)
}

/// Where a point given as fractions (0–1) of a window's inside is on the
/// screen.
pub fn window_point(window: isize, fx: f64, fy: f64) -> (i32, i32) {
    let (cx, cy) = background::center(window);
    match background::picture_size(window) {
        Some((w, h)) => (cx - w / 2 + (fx.clamp(0.0, 1.0) * w as f64) as i32, cy - h / 2 + (fy.clamp(0.0, 1.0) * h as f64) as i32),
        None => (cx, cy),
    }
}

/// One auto-clicker click (or key press), as set up.
// ── Capabilities for plugins ─────────────────────────────────────────────
//
// Single inputs, to the system (`None`, like real hardware) or straight to a
// window in the background (`Some(handle)`). The macro engine lives in the
// Macros plugin and drives these one by one (see docs/PLUGIN-API.md).

pub mod caps {
    use super::{Button, background, input};
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    /// Roblox ignores input while it thinks it's in the background, so a
    /// window gets "you're active" now and then before input reaches it.
    fn wake(window: isize) {
        static WOKEN: Mutex<Option<HashMap<isize, Instant>>> = Mutex::new(None);
        let Ok(mut woken) = WOKEN.lock() else { return };
        let map = woken.get_or_insert_with(HashMap::new);
        if map.get(&window).is_none_or(|t| t.elapsed() > Duration::from_millis(400)) {
            map.insert(window, Instant::now());
            background::activate(window);
        }
    }

    /// Where Pious keeps each background window's pointer (screen spot).
    fn pointers() -> &'static Mutex<HashMap<isize, (i32, i32)>> {
        static AT: std::sync::OnceLock<Mutex<HashMap<isize, (i32, i32)>>> = std::sync::OnceLock::new();
        AT.get_or_init(Default::default)
    }

    pub fn key(target: Option<isize>, code: &str, down: bool) -> Result<(), String> {
        if super::vk_of(code).is_none() {
            return Err(format!("Pious doesn't know the key {code}."));
        }
        match target {
            None => input::key(code, down),
            Some(w) => {
                wake(w);
                background::key(w, code, down);
            }
        }
        Ok(())
    }

    pub fn text(target: Option<isize>, text: &str) {
        match target {
            None => input::text(text),
            Some(w) => {
                wake(w);
                for c in text.chars() {
                    background::char(w, c);
                }
            }
        }
    }

    pub fn button(target: Option<isize>, button: Button, down: bool, at: Option<(i32, i32)>) {
        match target {
            None => input::button(button, down),
            Some(w) => {
                wake(w);
                let at = at.or_else(|| pointers().lock().ok().and_then(|p| p.get(&w).copied()));
                background::button(w, button, down, at);
            }
        }
    }

    pub fn move_pointer(target: Option<isize>, x: i32, y: i32, relative: bool) {
        match (target, relative) {
            (None, false) => input::move_to(x, y),
            (None, true) => input::move_by(x, y),
            (Some(w), _) => {
                let (x, y) = if relative {
                    let (cx, cy) = pointers().lock().ok().and_then(|p| p.get(&w).copied()).unwrap_or_else(|| background::center(w));
                    (cx + x, cy + y)
                } else {
                    (x, y)
                };
                if let Ok(mut p) = pointers().lock() {
                    p.insert(w, (x, y));
                }
                background::move_to(w, x, y);
            }
        }
    }

    pub fn scroll(target: Option<isize>, notches: i32, horizontal: bool, at: Option<(i32, i32)>) {
        match target {
            None => input::scroll(notches, horizontal),
            Some(w) => {
                wake(w);
                let at = at.or_else(|| pointers().lock().ok().and_then(|p| p.get(&w).copied()));
                background::scroll(w, notches, horizontal, at);
            }
        }
    }

    /// The color at a screen spot, or in a window's own picture (even when
    /// it's covered), as `#RRGGBB`.
    pub fn pixel(target: Option<isize>, x: i32, y: i32) -> Option<String> {
        let (r, g, b) = match target {
            None => input::pixel(x, y)?,
            Some(w) => background::pixel(w, x, y)?,
        };
        Some(format!("#{r:02X}{g:02X}{b:02X}"))
    }
}

/// A button by the name plugins use ("Left", "MouseLeft"…).
pub fn button_of(name: &str) -> Option<Button> {
    mouse_button(name).or_else(|| mouse_button(&format!("Mouse{name}")))
}

/// What was recorded, as plugins see it (`ms` from the first event).
pub fn events_json(events: &[(Instant, Event)]) -> Vec<serde_json::Value> {
    use serde_json::json;
    let start = events.first().map(|(t, _)| *t);
    events
        .iter()
        .map(|(t, e)| {
            let ms = start.map_or(0, |s| t.saturating_duration_since(s).as_millis() as u64);
            match *e {
                Event::Key { vk, down } => json!({ "ms": ms, "kind": "key", "code": code_of(vk), "down": down }),
                Event::Button { button, down, x, y } => {
                    json!({ "ms": ms, "kind": "button", "button": format!("{button:?}"), "down": down, "x": x, "y": y })
                }
                Event::Move { x, y } => json!({ "ms": ms, "kind": "move", "x": x, "y": y }),
                Event::Wheel { amount, horizontal } => json!({ "ms": ms, "kind": "scroll", "amount": amount, "horizontal": horizontal }),
            }
        })
        .collect()
}

// ── Recording ────────────────────────────────────────────────────────────

/// Something the user did while recording.
#[derive(Debug, Clone, Copy)]
pub enum Event {
    Key { vk: u16, down: bool },
    Button { button: Button, down: bool, x: i32, y: i32 },
    Move { x: i32, y: i32 },
    Wheel { amount: i32, horizontal: bool },
}

pub fn start_recording() -> Result<(), String> {
    input::start_recording()
}

/// Stops recording and returns what was recorded.
pub fn stop_recording() -> Vec<(Instant, Event)> {
    input::stop_recording()
}

/// The virtual key of a key name, for leaving the record hotkey out.
pub fn vk_of(code: &str) -> Option<u16> {
    input::vk_of(code).map(|(vk, _)| vk)
}

/// The key name of a virtual key ("KeyW" for 0x57).
pub fn code_of(vk: u16) -> Option<&'static str> {
    input::code_of(vk)
}

/// Tags every key and click Pious sends itself (`dwExtraInfo`), so its own
/// keyboard watching (key remaps, the input overlay) can tell them apart.
pub const MARK: usize = 0x5049_4F55;

/// Presses or lets go of a key or mouse button by name ("KeyE",
/// "MouseLeft"), like real hardware.
pub fn press(code: &str, down: bool) {
    match mouse_button(code) {
        Some(button) => input::button(button, down),
        None => input::key(code, down),
    }
}

/// Types a character, like real hardware.
/// Types a whole string at once (see `input::text`).
pub fn type_text(text: &str) {
    input::text(text);
}

/// The mouse button a name stands for ("MouseLeft", "MouseBack"…).
pub fn mouse_button(code: &str) -> Option<Button> {
    match code {
        "MouseLeft" => Some(Button::Left),
        "MouseRight" => Some(Button::Right),
        "MouseMiddle" => Some(Button::Middle),
        "MouseBack" => Some(Button::Back),
        "MouseForward" => Some(Button::Forward),
        _ => None,
    }
}

#[cfg(windows)]
mod input {
    use std::sync::Mutex;
    use std::time::Instant;

    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{CLR_INVALID, GetDC, GetPixel, ReleaseDC};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
        KEYEVENTF_SCANCODE, KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN,
        MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
        MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT, MapVirtualKeyW,
        SendInput,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetCursorPos, GetMessageW, HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT,
        PostThreadMessageW, SetCursorPos, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL,
        WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL,
        WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_XBUTTONDOWN,
        WM_XBUTTONUP,
    };

    use super::{Button, Event};

    /// (name, virtual key, extended).
    const KEYS: &[(&str, u16, bool)] = &[
        ("Space", 0x20, false),
        ("Enter", 0x0D, false),
        ("NumpadEnter", 0x0D, true),
        ("Tab", 0x09, false),
        ("Escape", 0x1B, false),
        ("Backspace", 0x08, false),
        ("ShiftLeft", 0xA0, false),
        ("ShiftRight", 0xA1, false),
        ("ControlLeft", 0xA2, false),
        ("ControlRight", 0xA3, true),
        ("AltLeft", 0xA4, false),
        ("AltRight", 0xA5, true),
        ("MetaLeft", 0x5B, true),
        ("MetaRight", 0x5C, true),
        ("CapsLock", 0x14, false),
        ("ArrowLeft", 0x25, true),
        ("ArrowUp", 0x26, true),
        ("ArrowRight", 0x27, true),
        ("ArrowDown", 0x28, true),
        ("Insert", 0x2D, true),
        ("Delete", 0x2E, true),
        ("Home", 0x24, true),
        ("End", 0x23, true),
        ("PageUp", 0x21, true),
        ("PageDown", 0x22, true),
        ("Minus", 0xBD, false),
        ("Equal", 0xBB, false),
        ("BracketLeft", 0xDB, false),
        ("BracketRight", 0xDD, false),
        ("Backslash", 0xDC, false),
        ("Semicolon", 0xBA, false),
        ("Quote", 0xDE, false),
        ("Backquote", 0xC0, false),
        ("Comma", 0xBC, false),
        ("Period", 0xBE, false),
        ("Slash", 0xBF, false),
        ("PrintScreen", 0x2C, true),
        ("ScrollLock", 0x91, false),
        ("Pause", 0x13, false),
        ("NumLock", 0x90, true),
        ("ContextMenu", 0x5D, true),
        ("NumpadMultiply", 0x6A, false),
        ("NumpadAdd", 0x6B, false),
        ("NumpadSubtract", 0x6D, false),
        ("NumpadDecimal", 0x6E, false),
        ("NumpadDivide", 0x6F, true),
    ];

    pub fn vk_of(code: &str) -> Option<(u16, bool)> {
        if let Some(&(_, vk, ext)) = KEYS.iter().find(|(name, ..)| *name == code) {
            return Some((vk, ext));
        }
        let single = |rest: &str| (rest.len() == 1).then(|| rest.as_bytes()[0]);
        if let Some(c) = code.strip_prefix("Key").and_then(single).filter(u8::is_ascii_uppercase) {
            return Some((c as u16, false));
        }
        if let Some(c) = code.strip_prefix("Digit").and_then(single).filter(u8::is_ascii_digit) {
            return Some((c as u16, false));
        }
        if let Some(c) = code.strip_prefix("Numpad").and_then(single).filter(u8::is_ascii_digit) {
            return Some((0x60 + (c - b'0') as u16, false));
        }
        if let Some(n) = code.strip_prefix('F').and_then(|n| n.parse::<u16>().ok()).filter(|n| (1..=24).contains(n)) {
            return Some((0x6F + n, false));
        }
        // The plain names hotkeys use for modifiers.
        match code {
            "Shift" => Some((0xA0, false)),
            "Control" => Some((0xA2, false)),
            "Alt" => Some((0xA4, false)),
            "Super" | "Meta" => Some((0x5B, true)),
            _ => None,
        }
    }

    pub fn code_of(vk: u16) -> Option<&'static str> {
        // Generic modifiers come through as their left-hand keys.
        let vk = match vk {
            0x10 => 0xA0,
            0x11 => 0xA2,
            0x12 => 0xA4,
            v => v,
        };
        if let Some((name, ..)) = KEYS.iter().find(|(_, v, ext)| *v == vk && !(*ext && vk == 0x0D)) {
            return Some(name);
        }
        const LETTERS: [&str; 26] = [
            "KeyA", "KeyB", "KeyC", "KeyD", "KeyE", "KeyF", "KeyG", "KeyH", "KeyI", "KeyJ", "KeyK", "KeyL", "KeyM",
            "KeyN", "KeyO", "KeyP", "KeyQ", "KeyR", "KeyS", "KeyT", "KeyU", "KeyV", "KeyW", "KeyX", "KeyY", "KeyZ",
        ];
        const DIGITS: [&str; 10] = ["Digit0", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8", "Digit9"];
        const PAD: [&str; 10] = ["Numpad0", "Numpad1", "Numpad2", "Numpad3", "Numpad4", "Numpad5", "Numpad6", "Numpad7", "Numpad8", "Numpad9"];
        const FKEYS: [&str; 24] = [
            "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "F13", "F14", "F15", "F16", "F17",
            "F18", "F19", "F20", "F21", "F22", "F23", "F24",
        ];
        match vk {
            0x41..=0x5A => Some(LETTERS[(vk - 0x41) as usize]),
            0x30..=0x39 => Some(DIGITS[(vk - 0x30) as usize]),
            0x60..=0x69 => Some(PAD[(vk - 0x60) as usize]),
            0x70..=0x87 => Some(FKEYS[(vk - 0x70) as usize]),
            _ => None,
        }
    }

    fn send(inputs: &[INPUT]) {
        unsafe {
            SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
        }
    }

    fn keyboard(vk: u16, scan: u16, flags: u32) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: super::MARK } },
        }
    }

    fn mouse(dx: i32, dy: i32, data: i32, flags: u32) -> INPUT {
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT { dx, dy, mouseData: data as u32, dwFlags: flags, time: 0, dwExtraInfo: super::MARK },
            },
        }
    }

    /// Presses or lets go of a key by its hardware scan code, which games
    /// (Roblox included) read.
    pub fn key(code: &str, down: bool) {
        let Some((vk, ext)) = vk_of(code) else { return };
        let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } as u16;
        let mut flags = KEYEVENTF_SCANCODE;
        if ext {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }
        if !down {
            flags |= KEYEVENTF_KEYUP;
        }
        send(&[keyboard(if scan == 0 { vk } else { 0 }, scan, if scan == 0 { flags & !KEYEVENTF_SCANCODE } else { flags })]);
    }

    /// Types text in one go. Characters beyond the basic plane (most
    /// emoji) are two UTF-16 halves; both go down before either comes up,
    /// in one batch, or programs like Roblox get a lone half and show
    /// nothing.
    pub fn text(text: &str) {
        let mut inputs = Vec::new();
        for c in text.chars() {
            let mut units = [0u16; 2];
            let units = c.encode_utf16(&mut units);
            inputs.extend(units.iter().map(|u| keyboard(0, *u, KEYEVENTF_UNICODE)));
            inputs.extend(units.iter().map(|u| keyboard(0, *u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)));
        }
        if !inputs.is_empty() {
            send(&inputs);
        }
    }

    pub fn button(button: Button, down: bool) {
        let (flags, data) = match (button, down) {
            (Button::Left, true) => (MOUSEEVENTF_LEFTDOWN, 0),
            (Button::Left, false) => (MOUSEEVENTF_LEFTUP, 0),
            (Button::Right, true) => (MOUSEEVENTF_RIGHTDOWN, 0),
            (Button::Right, false) => (MOUSEEVENTF_RIGHTUP, 0),
            (Button::Middle, true) => (MOUSEEVENTF_MIDDLEDOWN, 0),
            (Button::Middle, false) => (MOUSEEVENTF_MIDDLEUP, 0),
            (Button::Back, true) => (MOUSEEVENTF_XDOWN, 1),
            (Button::Back, false) => (MOUSEEVENTF_XUP, 1),
            (Button::Forward, true) => (MOUSEEVENTF_XDOWN, 2),
            (Button::Forward, false) => (MOUSEEVENTF_XUP, 2),
        };
        send(&[mouse(0, 0, data, flags)]);
    }

    pub fn move_to(x: i32, y: i32) {
        unsafe { SetCursorPos(x, y) };
    }

    /// A relative move, which is what turns a game's camera.
    pub fn move_by(dx: i32, dy: i32) {
        send(&[mouse(dx, dy, 0, MOUSEEVENTF_MOVE)]);
    }

    pub fn scroll(notches: i32, horizontal: bool) {
        send(&[mouse(0, 0, notches * 120, if horizontal { MOUSEEVENTF_HWHEEL } else { MOUSEEVENTF_WHEEL })]);
    }

    pub fn cursor() -> (i32, i32) {
        let mut point = POINT { x: 0, y: 0 };
        unsafe { GetCursorPos(&mut point) };
        (point.x, point.y)
    }

    pub fn pixel(x: i32, y: i32) -> Option<(u8, u8, u8)> {
        unsafe {
            let dc = GetDC(std::ptr::null_mut());
            let color = GetPixel(dc, x, y);
            ReleaseDC(std::ptr::null_mut(), dc);
            (color != CLR_INVALID).then(|| (color as u8, (color >> 8) as u8, (color >> 16) as u8))
        }
    }

    // Recording uses low-level hooks on a thread of its own; the hook
    // procedures can't carry context, so what they see goes here.
    static EVENTS: Mutex<Vec<(Instant, Event)>> = Mutex::new(Vec::new());
    static THREAD: Mutex<Option<(u32, std::thread::JoinHandle<()>)>> = Mutex::new(None);

    const INJECTED_KEY: u32 = 0x10;
    const INJECTED_MOUSE: u32 = 0x01;

    fn push(event: Event) {
        if let Ok(mut events) = EVENTS.lock() {
            events.push((Instant::now(), event));
        }
    }

    unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let info = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
            if info.flags & INJECTED_KEY == 0 {
                let down = matches!(wparam as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
                let up = matches!(wparam as u32, WM_KEYUP | WM_SYSKEYUP);
                if down || up {
                    push(Event::Key { vk: info.vkCode as u16, down });
                }
            }
        }
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
    }

    unsafe extern "system" fn mouse_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let info = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
            if info.flags & INJECTED_MOUSE == 0 {
                let (x, y) = (info.pt.x, info.pt.y);
                let xbutton = if (info.mouseData >> 16) == 1 { Button::Back } else { Button::Forward };
                let wheel = ((info.mouseData >> 16) as i16 as i32) / 120;
                let event = match wparam as u32 {
                    WM_MOUSEMOVE => Some(Event::Move { x, y }),
                    WM_LBUTTONDOWN => Some(Event::Button { button: Button::Left, down: true, x, y }),
                    WM_LBUTTONUP => Some(Event::Button { button: Button::Left, down: false, x, y }),
                    WM_RBUTTONDOWN => Some(Event::Button { button: Button::Right, down: true, x, y }),
                    WM_RBUTTONUP => Some(Event::Button { button: Button::Right, down: false, x, y }),
                    WM_MBUTTONDOWN => Some(Event::Button { button: Button::Middle, down: true, x, y }),
                    WM_MBUTTONUP => Some(Event::Button { button: Button::Middle, down: false, x, y }),
                    WM_XBUTTONDOWN => Some(Event::Button { button: xbutton, down: true, x, y }),
                    WM_XBUTTONUP => Some(Event::Button { button: xbutton, down: false, x, y }),
                    WM_MOUSEWHEEL if wheel != 0 => Some(Event::Wheel { amount: wheel, horizontal: false }),
                    WM_MOUSEHWHEEL if wheel != 0 => Some(Event::Wheel { amount: wheel, horizontal: true }),
                    _ => None,
                };
                if let Some(event) = event {
                    push(event);
                }
            }
        }
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
    }

    pub fn start_recording() -> Result<(), String> {
        let mut thread = THREAD.lock().map_err(|_| "Recording is unavailable.")?;
        if thread.is_some() {
            return Err("Already recording.".into());
        }
        EVENTS.lock().map(|mut e| e.clear()).ok();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let handle = std::thread::Builder::new()
            .name("macro-recorder".into())
            .spawn(move || unsafe {
                let module = GetModuleHandleW(std::ptr::null());
                let keyboard: HHOOK = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), module, 0);
                let mouse: HHOOK = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), module, 0);
                let ok = !keyboard.is_null() && !mouse.is_null();
                let _ = ready_tx.send((GetCurrentThreadId(), ok));
                if ok {
                    let mut msg: MSG = std::mem::zeroed();
                    while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
                if !keyboard.is_null() {
                    UnhookWindowsHookEx(keyboard);
                }
                if !mouse.is_null() {
                    UnhookWindowsHookEx(mouse);
                }
            })
            .map_err(|e| e.to_string())?;
        let (id, ok) = ready_rx.recv().map_err(|_| "Couldn't start recording.")?;
        if !ok {
            let _ = handle.join();
            return Err("Windows didn't let Pious watch the keyboard and mouse.".into());
        }
        *thread = Some((id, handle));
        Ok(())
    }

    pub fn stop_recording() -> Vec<(Instant, Event)> {
        let taken = THREAD.lock().ok().and_then(|mut t| t.take());
        if let Some((id, handle)) = taken {
            unsafe { PostThreadMessageW(id, WM_QUIT, 0, 0) };
            let _ = handle.join();
        }
        EVENTS.lock().map(|mut e| std::mem::take(&mut *e)).unwrap_or_default()
    }
}

/// Input posted straight to a window's message queue, the way Windows
/// delivers it to the window in front. The window doesn't need to be in
/// front, and the real pointer and keyboard are left alone.
#[cfg(windows)]
mod background {
    use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        ClientToScreen, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetPixel,
        ReleaseDC, ScreenToClient, SelectObject, CLR_INVALID,
    };
    use windows_sys::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MAPVK_VK_TO_VSC, MapVirtualKeyW};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetClientRect, GetWindowRect, IsWindow, PostMessageW, WA_ACTIVE, WM_ACTIVATE, WM_CHAR, WM_KEYDOWN, WM_KEYUP,
        WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL,
        WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETFOCUS, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_XBUTTONDOWN, WM_XBUTTONUP,
    };

    use super::Button;

    const MK_LBUTTON: usize = 0x0001;
    const MK_RBUTTON: usize = 0x0002;
    const MK_MBUTTON: usize = 0x0010;
    const MK_XBUTTON1: usize = 0x0020;
    const MK_XBUTTON2: usize = 0x0040;
    /// PrintWindow: include what DirectX draws (Windows 8.1 and later).
    const PW_RENDERFULLCONTENT: PRINT_WINDOW_FLAGS = 2;

    fn post(window: isize, message: u32, wparam: usize, lparam: isize) {
        let hwnd = window as HWND;
        unsafe {
            if IsWindow(hwnd) != 0 {
                PostMessageW(hwnd, message, wparam, lparam);
            }
        }
    }

    fn pack(x: i32, y: i32) -> isize {
        ((x as u16 as u32) | ((y as u16 as u32) << 16)) as i32 as isize
    }

    /// Screen coordinates → the window's own (client) coordinates.
    fn to_client(window: isize, x: i32, y: i32) -> (i32, i32) {
        let mut point = POINT { x, y };
        unsafe { ScreenToClient(window as HWND, &mut point) };
        (point.x, point.y)
    }

    /// The middle of the window, in screen coordinates.
    pub fn center(window: isize) -> (i32, i32) {
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        let mut origin = POINT { x: 0, y: 0 };
        unsafe {
            GetClientRect(window as HWND, &mut rect);
            ClientToScreen(window as HWND, &mut origin);
        }
        (origin.x + (rect.right - rect.left) / 2, origin.y + (rect.bottom - rect.top) / 2)
    }

    /// Tells the window it's the active one, so it takes the input.
    pub fn activate(window: isize) {
        post(window, WM_ACTIVATE, WA_ACTIVE as usize, 0);
        post(window, WM_SETFOCUS, 0, 0);
    }

    pub fn key(window: isize, code: &str, down: bool) {
        let Some((vk, ext)) = super::input::vk_of(code) else { return };
        let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } as isize;
        // Repeat count 1, the scan code, extended, and for a release the
        // "was down" and "going up" bits.
        let mut lparam = 1 | (scan << 16) | if ext { 1 << 24 } else { 0 };
        if !down {
            lparam |= 0xC000_0000u32 as i32 as isize;
        }
        let alt = matches!(vk, 0x12 | 0xA4 | 0xA5);
        let message = match (down, alt) {
            (true, false) => WM_KEYDOWN,
            (false, false) => WM_KEYUP,
            (true, true) => WM_SYSKEYDOWN,
            (false, true) => WM_SYSKEYUP,
        };
        post(window, message, vk as usize, lparam);
    }

    pub fn char(window: isize, c: char) {
        let mut units = [0u16; 2];
        for unit in c.encode_utf16(&mut units).iter() {
            post(window, WM_CHAR, *unit as usize, 1);
        }
    }

    pub fn move_to(window: isize, x: i32, y: i32) {
        let (cx, cy) = to_client(window, x, y);
        post(window, WM_MOUSEMOVE, 0, pack(cx, cy));
    }

    pub fn button(window: isize, button: Button, down: bool, at: Option<(i32, i32)>) {
        let (x, y) = at.unwrap_or_else(|| center(window));
        let (cx, cy) = to_client(window, x, y);
        let (message, flag, extra) = match (button, down) {
            (Button::Left, true) => (WM_LBUTTONDOWN, MK_LBUTTON, 0),
            (Button::Left, false) => (WM_LBUTTONUP, 0, 0),
            (Button::Right, true) => (WM_RBUTTONDOWN, MK_RBUTTON, 0),
            (Button::Right, false) => (WM_RBUTTONUP, 0, 0),
            (Button::Middle, true) => (WM_MBUTTONDOWN, MK_MBUTTON, 0),
            (Button::Middle, false) => (WM_MBUTTONUP, 0, 0),
            (Button::Back, true) => (WM_XBUTTONDOWN, MK_XBUTTON1, 1),
            (Button::Back, false) => (WM_XBUTTONUP, 0, 1),
            (Button::Forward, true) => (WM_XBUTTONDOWN, MK_XBUTTON2, 2),
            (Button::Forward, false) => (WM_XBUTTONUP, 0, 2),
        };
        // Games check where the pointer is right before a click.
        post(window, WM_MOUSEMOVE, 0, pack(cx, cy));
        post(window, message, flag | (extra << 16), pack(cx, cy));
    }

    pub fn scroll(window: isize, notches: i32, horizontal: bool, at: Option<(i32, i32)>) {
        let (x, y) = at.unwrap_or_else(|| center(window));
        let delta = (notches * 120) as i16 as u16 as usize;
        // Wheel messages carry screen coordinates.
        post(window, if horizontal { WM_MOUSEHWHEEL } else { WM_MOUSEWHEEL }, delta << 16, pack(x, y));
    }

    /// The size of the window's inside.
    pub fn picture_size(window: isize) -> Option<(i32, i32)> {
        use windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect;
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        (unsafe { GetClientRect(window as HWND, &mut rect) } != 0).then(|| (rect.right - rect.left, rect.bottom - rect.top))
    }

    /// What the window draws inside its frame, even when another window
    /// covers it: width, height and RGBA pixels.
    pub fn picture(window: isize) -> Option<(u32, u32, Vec<u8>)> {
        use windows_sys::Win32::Graphics::Gdi::{BI_RGB, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, GetDIBits};
        use windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect;
        // Only the inside of the window, with what DirectX draws.
        const PW_CLIENTONLY_FULL: PRINT_WINDOW_FLAGS = 1 | 2;
        let hwnd = window as HWND;
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        if unsafe { GetClientRect(hwnd, &mut rect) } == 0 {
            return None;
        }
        let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
        if w <= 0 || h <= 0 {
            return None;
        }
        unsafe {
            let screen = GetDC(std::ptr::null_mut());
            let dc = CreateCompatibleDC(screen);
            let bitmap = CreateCompatibleBitmap(screen, w, h);
            let old = SelectObject(dc, bitmap);
            let ok = PrintWindow(hwnd, dc, PW_CLIENTONLY_FULL) != 0;
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            let mut pixels = vec![0u8; (w * h * 4) as usize];
            let lines = if ok { GetDIBits(dc, bitmap, 0, h as u32, pixels.as_mut_ptr().cast(), &mut info, DIB_RGB_COLORS) } else { 0 };
            SelectObject(dc, old);
            DeleteObject(bitmap);
            DeleteDC(dc);
            ReleaseDC(std::ptr::null_mut(), screen);
            if lines == 0 {
                return None;
            }
            for px in pixels.chunks_exact_mut(4) {
                px.swap(0, 2);
                px[3] = 255;
            }
            Some((w as u32, h as u32, pixels))
        }
    }

    /// The color at a point (screen coordinates) of what the window draws,
    /// even when another window covers it.
    pub fn pixel(window: isize, x: i32, y: i32) -> Option<(u8, u8, u8)> {
        let hwnd = window as HWND;
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
            return None;
        }
        let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
        let (px, py) = (x - rect.left, y - rect.top);
        if w <= 0 || h <= 0 || px < 0 || py < 0 || px >= w || py >= h {
            return None;
        }
        unsafe {
            let screen = GetDC(std::ptr::null_mut());
            let dc = CreateCompatibleDC(screen);
            let bitmap = CreateCompatibleBitmap(screen, w, h);
            let old = SelectObject(dc, bitmap);
            let ok = PrintWindow(hwnd, dc, PW_RENDERFULLCONTENT) != 0;
            let color = if ok { GetPixel(dc, px, py) } else { CLR_INVALID };
            SelectObject(dc, old);
            DeleteObject(bitmap);
            DeleteDC(dc);
            ReleaseDC(std::ptr::null_mut(), screen);
            (color != CLR_INVALID).then(|| (color as u8, (color >> 8) as u8, (color >> 16) as u8))
        }
    }
}

#[cfg(not(windows))]
mod background {
    use super::Button;
    pub fn center(_w: isize) -> (i32, i32) {
        (0, 0)
    }
    pub fn activate(_w: isize) {}
    pub fn key(_w: isize, _code: &str, _down: bool) {}
    pub fn char(_w: isize, _c: char) {}
    pub fn move_to(_w: isize, _x: i32, _y: i32) {}
    pub fn button(_w: isize, _b: Button, _down: bool, _at: Option<(i32, i32)>) {}
    pub fn scroll(_w: isize, _n: i32, _h: bool, _at: Option<(i32, i32)>) {}
    pub fn pixel(_w: isize, _x: i32, _y: i32) -> Option<(u8, u8, u8)> {
        None
    }
    pub fn picture(_w: isize) -> Option<(u32, u32, Vec<u8>)> {
        None
    }
    pub fn picture_size(_w: isize) -> Option<(i32, i32)> {
        None
    }
}

#[cfg(not(windows))]
mod input {
    use super::{Button, Event};
    use std::time::Instant;
    pub fn vk_of(_code: &str) -> Option<(u16, bool)> {
        None
    }
    pub fn code_of(_vk: u16) -> Option<&'static str> {
        None
    }
    pub fn key(_code: &str, _down: bool) {}
    pub fn text(_t: &str) {}
    pub fn button(_b: Button, _down: bool) {}
    pub fn move_to(_x: i32, _y: i32) {}
    pub fn move_by(_x: i32, _y: i32) {}
    pub fn scroll(_n: i32, _h: bool) {}
    pub fn cursor() -> (i32, i32) {
        (0, 0)
    }
    pub fn pixel(_x: i32, _y: i32) -> Option<(u8, u8, u8)> {
        None
    }
    pub fn start_recording() -> Result<(), String> {
        Err("Recording macros needs Windows.".into())
    }
    pub fn stop_recording() -> Vec<(Instant, Event)> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_names_round_trip() {
        for code in ["KeyW", "Digit4", "F12", "Space", "ShiftLeft", "ArrowUp", "Numpad7", "Backquote"] {
            let vk = vk_of(code).unwrap();
            assert_eq!(input::code_of(vk), Some(code), "{code}");
        }
    }

    #[test]
    fn stops_promptly() {
        let stop = AtomicBool::new(true);
        struct NoHost;
        impl Host for NoHost {
            fn focus_roblox(&self) {}
        }
        let started = Instant::now();
        assert!(!play(&[Step::Wait { ms: 5000, random_ms: 0 }], 1.0, &stop, &NoHost, &Sink::System));
        assert!(started.elapsed() < Duration::from_millis(200));
    }
}
