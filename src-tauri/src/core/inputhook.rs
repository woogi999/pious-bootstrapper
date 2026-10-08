//! One low-level keyboard and mouse hook, shared by:
//!
//! - the input overlay (which keys and buttons are down right now),
//! - game keybinds (a key or button pressed in Roblox presses another),
//! - emoji shortcodes typed in Roblox (`:sob:` → 😭).
//!
//! Every key press on the PC passes through the hook, so it must answer
//! within microseconds: it only reads a shared configuration (never waiting
//! for a lock), flips atomics, and hands anything slow (sending the
//! replacement keys, showing the emoji list) to a worker thread.
//! Input Pious sends itself (tagged with [`crate::core::automation::MARK`])
//! is let through untouched.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};

/// What the hook should do. Swapped as a whole with [`configure`].
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Config {
    /// Keep track of what's held down (for the input overlay).
    pub watch: bool,
    /// Remaps (key name → key name, "" = blocked) for each Roblox process.
    pub remaps: HashMap<u32, HashMap<String, String>>,
    /// Turn :shortcodes: typed in these Roblox processes into emoji.
    pub emoji: HashSet<u32>,
    /// Keep track of the real mouse buttons (for the auto-clicker modes
    /// that follow your mouse).
    pub buttons: bool,
    /// Pious's own hotkeys (keys like "Shift+F8", and an ID), caught here
    /// while one of `hotkey_pids` is in front and kept from the game. A
    /// Windows hotkey can't do that for every key: F12 is often taken (it's
    /// reserved for debuggers), and Roblox would still see the key.
    pub hotkeys: Vec<(String, u32)>,
    pub hotkey_pids: HashSet<u32>,
}

impl Config {
    fn wanted(&self) -> bool {
        self.watch
            || self.buttons
            || self.remaps.values().any(|r| !r.is_empty())
            || !self.emoji.is_empty()
            || (!self.hotkeys.is_empty() && !self.hotkey_pids.is_empty())
    }
}

/// What the hook asks the rest of Pious to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Show the emoji list for what's been typed after the colon.
    EmojiList { query: String, matches: Vec<(String, String)>, selected: usize, pid: u32 },
    /// Hide the emoji list.
    EmojiHide,
    /// One of Pious's hotkeys was pressed (or let go) in a game.
    Hotkey { id: u32, pressed: bool },
}

/// Jobs for the worker thread (things too slow for the hook itself).
enum Job {
    Press(String, bool),
    /// Erase `erase` characters, then type `text`.
    Replace { erase: usize, text: String },
}

static CONFIG: Mutex<Option<Arc<Config>>> = Mutex::new(None);
static EVENTS: OnceLock<Mutex<Option<Sender<Event>>>> = OnceLock::new();
static JOBS: OnceLock<Sender<Job>> = OnceLock::new();

/// Bumped whenever what's held down changes.
static VERSION: AtomicU64 = AtomicU64::new(0);
/// Key presses and clicks so far (for "per second" rates).
static KEY_PRESSES: AtomicU64 = AtomicU64::new(0);
static CLICKS: AtomicU64 = AtomicU64::new(0);
/// Wheel notches so far, up and down.
static WHEEL_UP: AtomicU64 = AtomicU64::new(0);
static WHEEL_DOWN: AtomicU64 = AtomicU64::new(0);
/// What's held down, by name.
static HELD: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
/// The real mouse buttons (never Pious's own clicks): which are down, and
/// how many times each was pressed. Left, right, middle, back, forward.
const BUTTONS: [&str; 5] = ["MouseLeft", "MouseRight", "MouseMiddle", "MouseBack", "MouseForward"];
static BUTTONS_DOWN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static BUTTON_PRESSES: [AtomicU64; 5] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];

/// Whether a mouse button ("MouseLeft"…) is held down by hand right now.
pub fn physically_down(button: &str) -> bool {
    BUTTONS.iter().position(|b| *b == button).is_some_and(|i| BUTTONS_DOWN.load(Ordering::Relaxed) & (1 << i) != 0)
}

/// How many times a mouse button has been pressed by hand.
pub fn presses(button: &str) -> u64 {
    BUTTONS.iter().position(|b| *b == button).map(|i| BUTTON_PRESSES[i].load(Ordering::Relaxed)).unwrap_or(0)
}

fn track_button(code: &str, down: bool) {
    let Some(i) = BUTTONS.iter().position(|b| *b == code) else { return };
    if down {
        if BUTTONS_DOWN.fetch_or(1 << i, Ordering::Relaxed) & (1 << i) == 0 {
            BUTTON_PRESSES[i].fetch_add(1, Ordering::Relaxed);
        }
    } else {
        BUTTONS_DOWN.fetch_and(!(1 << i), Ordering::Relaxed);
    }
}
/// Keys whose press was remapped, and what they pressed instead, so their
/// release is remapped too (even if Roblox lost focus in between).
static REMAPPED: Mutex<Vec<(&'static str, String)>> = Mutex::new(Vec::new());
/// Keys held down that started a hotkey: (key, hotkey ID). Their repeats
/// and release are swallowed too.
static HOTKEY_HELD: Mutex<Vec<(&'static str, u32)>> = Mutex::new(Vec::new());

/// Whether `keys` ("Control+Shift+KeyK", "F12") is `code` with exactly
/// these modifiers held: [Control, Shift, Alt, Meta].
fn hotkey_matches(keys: &str, code: &str, held: [bool; 4]) -> bool {
    let mut want = [false; 4];
    let mut main = None;
    for part in keys.split('+').map(str::trim) {
        match part {
            "Control" | "Ctrl" | "CommandOrControl" | "CmdOrCtrl" => want[0] = true,
            "Shift" => want[1] = true,
            "Alt" | "Option" => want[2] = true,
            "Meta" | "Super" | "Win" | "Command" => want[3] = true,
            other => main = Some(other),
        }
    }
    main == Some(code) && want == held
}

/// Pious's hotkeys, in a game. Returns true to swallow the key.
fn on_hotkey(code: &'static str, down: bool, pid: u32, config: &Config) -> bool {
    if config.hotkeys.is_empty() || !config.hotkey_pids.contains(&pid) {
        // Released after the game lost focus: still let the action know.
        if !down {
            if let Ok(mut held) = HOTKEY_HELD.lock() {
                if let Some(i) = held.iter().position(|(c, _)| *c == code) {
                    let (_, id) = held.remove(i);
                    emit(Event::Hotkey { id, pressed: false });
                }
            }
        }
        return false;
    }
    let Ok(mut held) = HOTKEY_HELD.lock() else { return false };
    if let Some(i) = held.iter().position(|(c, _)| *c == code) {
        // A repeat while held, or the release.
        if !down {
            let (_, id) = held.remove(i);
            emit(Event::Hotkey { id, pressed: false });
        }
        return true;
    }
    if !down {
        return false;
    }
    let modifiers = imp::modifiers();
    match config.hotkeys.iter().find(|(keys, _)| hotkey_matches(keys, code, modifiers)) {
        Some((_, id)) => {
            held.push((code, *id));
            emit(Event::Hotkey { id: *id, pressed: true });
            true
        }
        None => false,
    }
}
/// Emoji shortcode typing in Roblox.
static TYPING: Mutex<Typing> = Mutex::new(Typing { active: false, query: String::new(), selected: 0, pid: 0, shown: false });

struct Typing {
    /// A colon was typed and a shortcode may be coming.
    active: bool,
    query: String,
    selected: usize,
    pid: u32,
    shown: bool,
}

/// What the input overlay draws.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Held {
    pub keys: Vec<&'static str>,
    pub key_presses: u64,
    pub clicks: u64,
    pub wheel_up: u64,
    pub wheel_down: u64,
}

/// What's held down right now, and the running counts.
pub fn held() -> Held {
    Held {
        keys: HELD.lock().map(|h| h.clone()).unwrap_or_default(),
        key_presses: KEY_PRESSES.load(Ordering::Relaxed),
        clicks: CLICKS.load(Ordering::Relaxed),
        wheel_up: WHEEL_UP.load(Ordering::Relaxed),
        wheel_down: WHEEL_DOWN.load(Ordering::Relaxed),
    }
}

/// Changes whenever [`held`] would say something new.
pub fn version() -> u64 {
    VERSION.load(Ordering::Relaxed)
}

/// Replaces what the hook does, starting or stopping it as needed.
/// `events` receives what the hook asks for (the emoji list).
pub fn configure(config: Config, events: &Sender<Event>) {
    {
        let slot = EVENTS.get_or_init(|| Mutex::new(None));
        *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(events.clone());
    }
    let wanted = config.wanted();
    {
        let mut current = CONFIG.lock().unwrap_or_else(|e| e.into_inner());
        if current.as_deref() == Some(&config) {
            return;
        }
        *current = Some(Arc::new(config));
    }
    imp::run(wanted);
    if !wanted {
        if let Ok(mut held) = HELD.lock() {
            if !held.is_empty() {
                held.clear();
                VERSION.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

fn config() -> Option<Arc<Config>> {
    // Never wait: if it's being swapped right now, act as if unset.
    CONFIG.try_lock().ok().and_then(|c| c.clone())
}

fn emit(event: Event) {
    if let Some(slot) = EVENTS.get() {
        if let Ok(sender) = slot.try_lock() {
            if let Some(sender) = sender.as_ref() {
                let _ = sender.send(event);
            }
        }
    }
}

fn job(job: Job) {
    let sender = JOBS.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = std::thread::Builder::new().name("input-jobs".into()).spawn(move || worker(rx));
        tx
    });
    let _ = sender.send(job);
}

fn worker(jobs: Receiver<Job>) {
    use crate::core::automation;
    while let Ok(job) = jobs.recv() {
        match job {
            Job::Press(code, down) => automation::press(&code, down),
            Job::Replace { erase, text } => {
                for _ in 0..erase {
                    automation::press("Backspace", true);
                    automation::press("Backspace", false);
                }
                for c in text.chars() {
                    automation::type_char(c);
                }
            }
        }
    }
}

fn set_held(code: &'static str, down: bool) {
    let Ok(mut held) = HELD.try_lock() else { return };
    let changed = if down {
        if held.contains(&code) {
            false
        } else {
            held.push(code);
            true
        }
    } else {
        let before = held.len();
        held.retain(|c| *c != code);
        before != held.len()
    };
    if changed {
        VERSION.fetch_add(1, Ordering::Relaxed);
    }
}

/// A key or button went down or up physically. Returns true to swallow
/// it (it was remapped).
fn on_input(code: &'static str, down: bool, pid: u32, config: &Config) -> bool {
    track_button(code, down);
    if !code.starts_with("Mouse") && on_hotkey(code, down, pid, config) {
        return true;
    }
    if config.watch {
        if down && !HELD.try_lock().is_ok_and(|h| h.contains(&code)) {
            if code.starts_with("Mouse") {
                CLICKS.fetch_add(1, Ordering::Relaxed);
            } else {
                KEY_PRESSES.fetch_add(1, Ordering::Relaxed);
            }
        }
        set_held(code, down);
    }
    // A key already remapped on its way down is remapped on its way up.
    if let Ok(mut remapped) = REMAPPED.try_lock() {
        if let Some(i) = remapped.iter().position(|(from, _)| *from == code) {
            if down {
                // Held down (key repeat): repeat the replacement.
                let to = remapped[i].1.clone();
                if !to.is_empty() {
                    job(Job::Press(to, true));
                }
            } else {
                let (_, to) = remapped.remove(i);
                if !to.is_empty() {
                    job(Job::Press(to, false));
                }
            }
            return true;
        }
        if down {
            if let Some(to) = config.remaps.get(&pid).and_then(|r| r.get(code)) {
                remapped.push((code, to.clone()));
                if !to.is_empty() {
                    job(Job::Press(to.clone(), true));
                }
                return true;
            }
        }
    }
    false
}

/// A wheel notch. Returns true to swallow it (it was remapped).
fn on_wheel(up: bool, pid: u32, config: &Config) -> bool {
    if config.watch {
        if up { &WHEEL_UP } else { &WHEEL_DOWN }.fetch_add(1, Ordering::Relaxed);
        VERSION.fetch_add(1, Ordering::Relaxed);
    }
    let code = if up { "WheelUp" } else { "WheelDown" };
    match config.remaps.get(&pid).and_then(|r| r.get(code)) {
        Some(to) => {
            if !to.is_empty() {
                job(Job::Press(to.clone(), true));
                job(Job::Press(to.clone(), false));
            }
            true
        }
        None => false,
    }
}

/// How many emoji the list shows.
const LIST: usize = 8;

fn list(typing: &mut Typing) {
    let matches: Vec<(String, String)> =
        crate::core::emoji::search(&typing.query, LIST).into_iter().map(|(n, e)| (n.to_owned(), e.to_owned())).collect();
    if typing.query.is_empty() || matches.is_empty() {
        hide(typing);
        return;
    }
    typing.selected = typing.selected.min(matches.len() - 1);
    typing.shown = true;
    emit(Event::EmojiList { query: typing.query.clone(), matches, selected: typing.selected, pid: typing.pid });
}

fn hide(typing: &mut Typing) {
    if std::mem::take(&mut typing.shown) {
        emit(Event::EmojiHide);
    }
}

fn reset(typing: &mut Typing) {
    typing.active = false;
    typing.query.clear();
    typing.selected = 0;
    hide(typing);
}

/// Typing in Roblox: watches for `:shortcode` and offers emoji. Returns
/// true to swallow the key (Enter, Tab or arrows used by the list).
fn on_typing(vk: u16, typed: Option<char>, pid: u32) -> bool {
    const BACKSPACE: u16 = 0x08;
    const TAB: u16 = 0x09;
    const ENTER: u16 = 0x0D;
    const ESCAPE: u16 = 0x1B;
    const UP: u16 = 0x26;
    const DOWN: u16 = 0x28;
    // Modifiers alone change nothing.
    if matches!(vk, 0x10 | 0x11 | 0x12 | 0xA0..=0xA5 | 0x14) {
        return false;
    }
    let Ok(mut typing) = TYPING.try_lock() else { return false };
    if typing.active && typing.pid != pid {
        reset(&mut typing);
    }
    match (vk, typed) {
        (_, Some(':')) => {
            // The closing colon of a full shortcode: swap it for the emoji
            // (the colon itself goes through first, then everything is erased).
            if typing.active {
                if let Some(emoji) = crate::core::emoji::exact(&typing.query) {
                    job(Job::Replace { erase: typing.query.chars().count() + 2, text: emoji.to_owned() });
                    reset(&mut typing);
                    return false;
                }
            }
            reset(&mut typing);
            typing.active = true;
            typing.pid = pid;
            false
        }
        _ if !typing.active => false,
        (ENTER | TAB, _) if typing.shown => {
            let picked = crate::core::emoji::search(&typing.query, LIST).get(typing.selected).map(|(_, e)| (*e).to_owned());
            let erase = typing.query.chars().count() + 1;
            reset(&mut typing);
            match picked {
                Some(emoji) => {
                    job(Job::Replace { erase, text: emoji });
                    true
                }
                None => false,
            }
        }
        (UP | DOWN, _) if typing.shown => {
            let count = crate::core::emoji::search(&typing.query, LIST).len().max(1);
            typing.selected = if vk == UP { (typing.selected + count - 1) % count } else { (typing.selected + 1) % count };
            list(&mut typing);
            true
        }
        (BACKSPACE, _) => {
            if typing.query.pop().is_none() {
                // The colon itself was erased.
                reset(&mut typing);
            } else {
                list(&mut typing);
            }
            false
        }
        (ESCAPE, _) => {
            reset(&mut typing);
            false
        }
        (_, Some(c)) if crate::core::emoji::is_shortcode_char(c) && typing.query.len() < 40 => {
            typing.query.push(c);
            typing.selected = 0;
            list(&mut typing);
            false
        }
        _ => {
            reset(&mut typing);
            false
        }
    }
}

/// Clicking somewhere ends any shortcode being typed.
fn on_click() {
    if let Ok(mut typing) = TYPING.try_lock() {
        if typing.active {
            reset(&mut typing);
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::sync::Mutex;

    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, GetKeyState, GetKeyboardLayout, ToUnicodeEx};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId, KBDLLHOOKSTRUCT,
        MSG, MSLLHOOKSTRUCT, PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
        WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
        WM_MOUSEWHEEL, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_XBUTTONDOWN, WM_XBUTTONUP,
    };

    use crate::core::automation::{MARK, code_of};

    /// The hook thread, while it runs.
    static THREAD: Mutex<Option<(u32, std::thread::JoinHandle<()>)>> = Mutex::new(None);

    const INJECTED_KEY: u32 = 0x10;
    const INJECTED_MOUSE: u32 = 0x01;

    /// Starts or stops the hook thread.
    pub fn run(wanted: bool) {
        let mut thread = THREAD.lock().unwrap_or_else(|e| e.into_inner());
        if wanted == thread.is_some() {
            return;
        }
        if let Some((id, handle)) = thread.take() {
            unsafe { PostThreadMessageW(id, WM_QUIT, 0, 0) };
            let _ = handle.join();
            return;
        }
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let Ok(handle) = std::thread::Builder::new().name("input-hook".into()).spawn(move || unsafe {
            let module = GetModuleHandleW(std::ptr::null());
            let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), module, 0);
            let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), module, 0);
            let _ = ready_tx.send(GetCurrentThreadId());
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if !keyboard.is_null() {
                UnhookWindowsHookEx(keyboard);
            }
            if !mouse.is_null() {
                UnhookWindowsHookEx(mouse);
            }
        }) else {
            return;
        };
        if let Ok(id) = ready_rx.recv() {
            *thread = Some((id, handle));
        }
    }

    /// The modifier keys held right now: [Control, Shift, Alt, Windows].
    pub fn modifiers() -> [bool; 4] {
        let down = |vk: u16| unsafe { GetAsyncKeyState(vk as i32) } as u16 & 0x8000 != 0;
        [down(0x11), down(0x10), down(0x12), down(0x5B) || down(0x5C)]
    }

    fn foreground_pid() -> u32 {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(GetForegroundWindow(), &mut pid) };
        pid
    }

    /// The character a key types with the current Shift and Caps Lock, in
    /// the foreground window's keyboard layout.
    fn typed(vk: u32, scan: u32) -> Option<char> {
        unsafe {
            let mut state = [0u8; 256];
            if GetAsyncKeyState(0x10) < 0 {
                state[0x10] = 0x80;
            }
            if GetKeyState(0x14) & 1 != 0 {
                state[0x14] = 0x01;
            }
            // Ctrl or Alt held: a shortcut, not typing.
            if GetAsyncKeyState(0x11) < 0 || GetAsyncKeyState(0x12) < 0 {
                return None;
            }
            let thread = GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut());
            let layout = GetKeyboardLayout(thread);
            let mut out = [0u16; 4];
            // Flag 4: don't disturb the keyboard state (dead keys keep working).
            let n = ToUnicodeEx(vk, scan, state.as_ptr(), out.as_mut_ptr(), out.len() as i32, 4, layout);
            (n == 1).then(|| char::from_u32(out[0] as u32)).flatten()
        }
    }

    unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let info = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
            let ours = info.dwExtraInfo == MARK;
            let injected = info.flags & INJECTED_KEY != 0;
            let down = matches!(wparam as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
            let up = matches!(wparam as u32, WM_KEYUP | WM_SYSKEYUP);
            if !ours && !injected && (down || up) {
                if let Some(config) = super::config() {
                    let pid = foreground_pid();
                    let vk = info.vkCode as u16;
                    if down && config.emoji.contains(&pid) {
                        let typed = typed(info.vkCode, info.scanCode);
                        if super::on_typing(vk, typed, pid) {
                            return 1;
                        }
                    }
                    if let Some(name) = code_of(vk) {
                        if super::on_input(name, down, pid, &config) {
                            return 1;
                        }
                    }
                }
            }
        }
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
    }

    unsafe extern "system" fn mouse_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let info = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
            let ours = info.dwExtraInfo == MARK;
            let injected = info.flags & INJECTED_MOUSE != 0;
            if !ours && !injected {
                if let Some(config) = super::config() {
                    let x = if (info.mouseData >> 16) == 1 { "MouseBack" } else { "MouseForward" };
                    let button = match wparam as u32 {
                        WM_LBUTTONDOWN => Some(("MouseLeft", true)),
                        WM_LBUTTONUP => Some(("MouseLeft", false)),
                        WM_RBUTTONDOWN => Some(("MouseRight", true)),
                        WM_RBUTTONUP => Some(("MouseRight", false)),
                        WM_MBUTTONDOWN => Some(("MouseMiddle", true)),
                        WM_MBUTTONUP => Some(("MouseMiddle", false)),
                        WM_XBUTTONDOWN => Some((x, true)),
                        WM_XBUTTONUP => Some((x, false)),
                        _ => None,
                    };
                    let pid = foreground_pid();
                    if let Some((name, down)) = button {
                        if down {
                            super::on_click();
                        }
                        if super::on_input(name, down, pid, &config) {
                            return 1;
                        }
                    } else if wparam as u32 == WM_MOUSEWHEEL {
                        let delta = (info.mouseData >> 16) as i16;
                        if delta != 0 && super::on_wheel(delta > 0, pid, &config) {
                            return 1;
                        }
                    }
                }
            }
        }
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn run(_wanted: bool) {}
    pub fn modifiers() -> [bool; 4] {
        [false; 4]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkeys_need_their_exact_modifiers() {
        assert!(hotkey_matches("F12", "F12", [false; 4]));
        assert!(!hotkey_matches("F12", "F12", [false, true, false, false]));
        assert!(hotkey_matches("Shift+F8", "F8", [false, true, false, false]));
        assert!(!hotkey_matches("Shift+F8", "F8", [false; 4]));
        assert!(hotkey_matches("Control+Shift+KeyK", "KeyK", [true, true, false, false]));
    }

    #[test]
    fn remaps_press_and_release_together() {
        let config = Config {
            remaps: [(7, [("KeyQ".to_owned(), "".to_owned())].into_iter().collect())].into_iter().collect(),
            ..Config::default()
        };
        // Blocked in the remapped window, both ways.
        assert!(on_input("KeyQ", true, 7, &config));
        assert!(on_input("KeyQ", false, 9, &config));
        // Elsewhere it goes through.
        assert!(!on_input("KeyQ", true, 9, &config));
        assert!(!on_input("KeyQ", false, 9, &config));
    }
}
