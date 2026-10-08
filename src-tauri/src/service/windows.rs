//! Pious's other windows: the floating startup logo, the chat window and
//! the built-in browser.

use std::time::Duration;

use serde_json::json;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use uuid::Uuid;

use super::{Service, Shared, Tone};
use crate::core::model::LinkTarget;

pub const SPLASH_WINDOW: &str = "splash";
pub const CHAT_WINDOW: &str = "chat";
pub const BROWSER_WINDOW: &str = "browser";

impl Service {
    /// The startup logo finished: show the app (unless it started in the
    /// tray) and close the logo's window.
    pub fn splash_done(self: &Shared) {
        let first = !std::mem::replace(&mut self.read().splash_done, true);
        if !first {
            return;
        }
        let background = std::env::args().any(|a| a == "--background");
        // The logo has already faded out: its window goes first, then the
        // app appears on its own.
        if let Some(splash) = self.app().get_webview_window(SPLASH_WINDOW) {
            let _ = splash.close();
        }
        // Started in the tray: the window is made when it's first opened.
        if !background {
            self.open_main(true);
        }
    }

    /// Never leave the app hidden behind a logo that didn't finish.
    pub async fn splash_failsafe(self: Shared) {
        tokio::time::sleep(Duration::from_secs(10)).await;
        self.splash_done();
    }

    /// Opens (or brings forward) the chat window, on a conversation with
    /// `user` when given.
    pub fn open_chat_window(&self, account: Option<Uuid>, user: Option<u64>) -> Result<(), String> {
        // One lock at a time: two `self.read()` guards in one statement
        // deadlock (the first lives until the statement ends).
        let account = account.or_else(|| {
            let s = self.read();
            s.friends.account.or(s.bootstrapper.active_account)
        });
        let target = json!({ "account": account, "user": user });
        let app = self.app().clone();
        if let Some(window) = app.get_webview_window(CHAT_WINDOW) {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
            let _ = app.emit_to(CHAT_WINDOW, "chat-open", target);
            return Ok(());
        }
        let hash = format!(
            "chat:{}:{}",
            account.map(|a| a.to_string()).unwrap_or_default(),
            user.map(|u| u.to_string()).unwrap_or_default()
        );
        let window = WebviewWindowBuilder::new(&app, CHAT_WINDOW, WebviewUrl::App(format!("index.html#{hash}").into()))
            .title("Pious Chat")
            .inner_size(900.0, 620.0)
            .min_inner_size(620.0, 420.0)
            .decorations(false)
            .transparent(true)
            .shadow(true)
            .visible(true)
            .build()
            .map_err(|e| e.to_string())?;
        let _ = window.set_focus();
        super::settings::apply_window(self);
        Ok(())
    }

    /// Opens a web link where the user wants links to open.
    pub fn open_web_link(&self, url: &str) {
        let web = url.starts_with("https://") || url.starts_with("http://");
        if !web {
            self.toast(Tone::Negative, "That isn't a web link.");
            return;
        }
        if self.read().bootstrapper.preferences.link_target == LinkTarget::Browser {
            if open::that_detached(url).is_err() {
                self.toast(Tone::Negative, "Couldn't open your browser.");
            }
            return;
        }
        let Ok(parsed) = url.parse::<tauri::Url>() else {
            self.toast(Tone::Negative, "That link doesn't work.");
            return;
        };
        let app = self.app().clone();
        let me_url = url.to_owned();
        // Windows are made on the main thread (WebView2 needs it).
        let _ = self.app().run_on_main_thread(move || {
            if let Some(window) = app.get_webview_window(BROWSER_WINDOW) {
                let _ = window.navigate(parsed);
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
                return;
            }
            let built = WebviewWindowBuilder::new(&app, BROWSER_WINDOW, WebviewUrl::External(parsed))
                .title("Pious Browser")
                .inner_size(1180.0, 800.0)
                .min_inner_size(480.0, 360.0)
                .center()
                .on_document_title_changed(|window, title| {
                    let _ = window.set_title(&format!("{title} · Pious Browser"));
                })
                .build();
            if built.is_err() {
                let _ = open::that_detached(&me_url);
            }
        });
    }
}

/// The thread running the window event loop, so code that may run there
/// knows not to build a window inline (WebView2 can deadlock if a window
/// is built inside one of its own callbacks).
static MAIN_THREAD: std::sync::OnceLock<std::thread::ThreadId> = std::sync::OnceLock::new();

/// Called once from setup, which runs on the event loop's thread.
pub fn remember_main_thread() {
    let _ = MAIN_THREAD.set(std::thread::current().id());
}

/// Builds the main window (hidden). It isn't in tauri.conf.json: a window
/// that exists but is hidden while the startup logo plays shows up as a
/// blank white frame behind the logo (a WebView2 quirk), so it's only made
/// once the logo is done.
fn build_main(app: &tauri::AppHandle) -> tauri::Result<tauri::WebviewWindow> {
    if let Some(window) = app.get_webview_window("main") {
        return Ok(window);
    }
    WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Pious")
        .inner_size(1360.0, 860.0)
        .min_inner_size(1040.0, 660.0)
        .center()
        .decorations(false)
        .transparent(true)
        .shadow(true)
        .disable_drag_drop_handler()
        .visible(false)
        .build()
}

/// Shows a window and brings it forward.
pub fn reveal(window: &tauri::WebviewWindow) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

impl Service {
    /// Makes the main window if it doesn't exist yet (applying the look
    /// settings to it), then shows it when `show`.
    pub fn open_main(self: &Shared, show: bool) {
        if let Some(window) = self.app().get_webview_window("main") {
            if show {
                reveal(&window);
            }
            return;
        }
        let me = self.clone();
        let make = move || {
            match build_main(me.app()) {
                Ok(window) => {
                    super::settings::apply_window(&me);
                    if show {
                        reveal(&window);
                    }
                    let _ = me.app().emit_to("main", "app-shown", ());
                }
                Err(error) => me.toast(Tone::Negative, format!("Couldn't open Pious's window: {error}")),
            }
        };
        if MAIN_THREAD.get() == Some(&std::thread::current().id()) {
            std::thread::spawn(make);
        } else {
            make();
        }
    }
}
