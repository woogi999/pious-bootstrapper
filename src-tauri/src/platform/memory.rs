//! Keeping Pious small while it's in the background.
//!
//! Every Pious window is a WebView2 (a browser of its own), and those are
//! what take the memory. While a window is hidden, minimized or left
//! unfocused for a while, it's asked to use as little as it can (WebView2's
//! low memory target: it drops caches and pages out what it can), and
//! Pious's own process hands back the memory it isn't using. Back in front,
//! the window returns to normal straight away.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, WebviewWindow};

/// How long a visible window may sit unfocused before it's made small.
const UNFOCUSED_GRACE: Duration = Duration::from_secs(45);
/// Windows that are closed for good after staying hidden this long, and
/// made again when they're next needed: the main window (in the tray) and
/// the pop-ups over games. The in-game overlay stays, so it opens at once.
const RELEASE: [(&str, Duration); 5] = [
    ("main", Duration::from_secs(120)),
    ("emoji", Duration::from_secs(60)),
    ("inputs", Duration::from_secs(60)),
    ("stats", Duration::from_secs(60)),
    ("notify", Duration::from_secs(60)),
];

#[derive(Clone, Copy, PartialEq)]
enum Level {
    Normal,
    Low,
}

/// The level each window was last given, and since when it's been
/// unfocused.
static STATE: Mutex<Option<HashMap<String, (Level, Option<Instant>)>>> = Mutex::new(None);

/// A window was focused: back to normal now (not on the next check).
pub fn focused(window: &WebviewWindow) {
    set(window, Level::Normal);
}

/// Checks every window every few seconds, for as long as Pious runs.
pub async fn watch(app: AppHandle) {
    let mut hidden_since: HashMap<&'static str, Instant> = HashMap::new();
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let mut trimmed = false;
        let mut all_small = true;
        // Windows hidden long enough go entirely (their whole browser).
        for (label, after) in RELEASE {
            match app.get_webview_window(label) {
                Some(window) if !window.is_visible().unwrap_or(true) => {
                    if hidden_since.entry(label).or_insert_with(Instant::now).elapsed() >= after {
                        hidden_since.remove(label);
                        if let Ok(mut state) = STATE.lock() {
                            if let Some(map) = state.as_mut() {
                                map.remove(label);
                            }
                        }
                        let _ = window.destroy();
                        trimmed = true;
                    }
                }
                _ => {
                    hidden_since.remove(label);
                }
            }
        }
        for (label, window) in app.webview_windows() {
            let visible = window.is_visible().unwrap_or(true);
            let minimized = window.is_minimized().unwrap_or(false);
            let has_focus = window.is_focused().unwrap_or(false);
            let unfocused_since = {
                let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
                let entry = state.get_or_insert_with(HashMap::new).entry(label.clone()).or_insert((Level::Normal, None));
                if has_focus || !visible {
                    entry.1 = None;
                } else if entry.1.is_none() {
                    entry.1 = Some(Instant::now());
                }
                entry.1
            };
            let wanted = if !visible || minimized || unfocused_since.is_some_and(|t| t.elapsed() >= UNFOCUSED_GRACE) {
                Level::Low
            } else {
                Level::Normal
            };
            if wanted == Level::Normal {
                all_small = false;
            }
            if set(&window, wanted) && wanted == Level::Low {
                trimmed = true;
            }
        }
        // Everything's in the background: give back what Pious itself
        // isn't using (it comes back as it's needed).
        if trimmed && all_small {
            trim();
        }
    }
}

/// Gives a window a level; whether it changed.
fn set(window: &WebviewWindow, level: Level) -> bool {
    {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let entry = state.get_or_insert_with(HashMap::new).entry(window.label().to_owned()).or_insert((Level::Normal, None));
        if entry.0 == level {
            return false;
        }
        entry.0 = level;
    }
    apply(window, level);
    true
}

#[cfg(windows)]
fn apply(window: &WebviewWindow, level: Level) {
    let _ = window.with_webview(move |webview| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL, ICoreWebView2_19,
        };
        use windows::core::Interface;
        let Ok(core) = webview.controller().CoreWebView2() else { return };
        // Older WebView2 runtimes don't have it; they just stay as they are.
        if let Ok(core) = core.cast::<ICoreWebView2_19>() {
            let target = match level {
                Level::Low => COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
                Level::Normal => COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
            };
            let _ = core.SetMemoryUsageTargetLevel(target);
        }
    });
}

#[cfg(not(windows))]
fn apply(_window: &WebviewWindow, _level: Level) {}

/// Hands back the memory Pious and its browsers (WebView2's processes,
/// which Pious started) aren't using right now. It comes back as needed.
#[cfg(windows)]
fn trim() {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcessId, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA, SetProcessWorkingSetSize,
    };
    unsafe {
        // Every process and its parent, to find Pious's descendants.
        let mut parents: Vec<(u32, u32)> = Vec::new();
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot != INVALID_HANDLE_VALUE {
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut more = Process32FirstW(snapshot, &mut entry) != 0;
            while more {
                parents.push((entry.th32ProcessID, entry.th32ParentProcessID));
                more = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
        }
        let mut ours = vec![GetCurrentProcessId()];
        let mut i = 0;
        while i < ours.len() {
            let parent = ours[i];
            ours.extend(parents.iter().filter(|(pid, p)| *p == parent && *pid != parent && !ours.contains(pid)).map(|(pid, _)| *pid).collect::<Vec<_>>());
            i += 1;
        }
        for pid in ours {
            let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if !process.is_null() {
                SetProcessWorkingSetSize(process, usize::MAX, usize::MAX);
                CloseHandle(process);
            }
        }
    }
}

#[cfg(not(windows))]
fn trim() {}
