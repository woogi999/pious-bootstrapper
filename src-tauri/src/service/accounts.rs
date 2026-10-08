//! Accounts: signing in (Roblox's own page, Quick Login or a session
//! token), and managing the saved profiles.

use std::time::Duration;

use serde::Serialize;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use uuid::Uuid;

use super::{Service, Shared, SignIn, Tone};
use crate::core::model::Account;
use crate::core::roblox::{self, QuickLoginStatus, UserInfo};
use crate::core::{credentials, store};

const SIGN_IN_WINDOW: &str = "roblox-sign-in";

/// What a Quick Login check found.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QuickLoginUpdate {
    /// Still waiting (or the user entered the code and must confirm).
    Waiting { status: QuickLoginStatus },
    /// Signed in; the account was added.
    Done,
    /// The code expired or was cancelled; ask for a new one.
    Expired { status: QuickLoginStatus },
}

/// A Roblox sign-in found in a web browser.
#[derive(Debug, Clone, Serialize)]
pub struct BrowserSession {
    pub browser: String,
    pub user_id: u64,
    pub username: String,
    pub display_name: String,
    /// Already one of the accounts in Pious.
    pub added: bool,
    #[serde(skip)]
    pub token: String,
}

/// What looking through the browsers found, for the window.
#[derive(Debug, Clone, Serialize)]
pub struct BrowserSearch {
    pub found: usize,
    pub problems: Vec<String>,
}

impl Service {
    /// Looks for Roblox sign-ins saved in the user's browsers and checks
    /// which accounts they belong to.
    pub async fn find_browser_sessions(&self) -> Result<BrowserSearch, String> {
        let search = tokio::task::spawn_blocking(crate::core::browsers::search).await.map_err(|e| e.to_string())?;
        let checks = search.found.iter().map(|f| async move { (f, roblox::authenticated_user(&f.token).await) });
        let mut sessions = Vec::new();
        let mut problems = search.problems.clone();
        for (found, result) in futures::future::join_all(checks).await {
            match result {
                Ok(user) => {
                    if sessions.iter().any(|s: &BrowserSession| s.user_id == user.id) {
                        continue;
                    }
                    let added = self.read().bootstrapper.accounts.iter().any(|a| a.user_id == user.id && !a.needs_sign_in);
                    sessions.push(BrowserSession {
                        browser: found.browser.clone(),
                        user_id: user.id,
                        username: user.username,
                        display_name: user.display_name,
                        added,
                        token: found.token.clone(),
                    });
                }
                Err(_) => problems.push(format!("{}'s Roblox sign-in has expired.", found.browser)),
            }
        }
        let count = sessions.len();
        self.mutate(|s| s.browser_sessions = sessions);
        Ok(BrowserSearch { found: count, problems })
    }

    /// Adds the account of a sign-in found in a browser.
    pub async fn add_browser_session(self: &Shared, user_id: u64, reauth: Option<Uuid>) -> Result<(), String> {
        let token = self
            .read()
            .browser_sessions
            .iter()
            .find(|s| s.user_id == user_id)
            .map(|s| s.token.clone())
            .ok_or("Look through your browsers again.")?;
        self.add_session(token, reauth).await?;
        self.mutate(|s| {
            for session in &mut s.browser_sessions {
                if session.user_id == user_id {
                    session.added = true;
                }
            }
        });
        Ok(())
    }

    /// Checks a session with Roblox and saves the account it belongs to.
    pub async fn add_session(self: &Shared, token: String, reauth: Option<Uuid>) -> Result<UserInfo, String> {
        let token = token.trim().trim_start_matches(".ROBLOSECURITY=").trim().to_owned();
        if token.is_empty() {
            return Err("Paste your session token to continue.".into());
        }
        let user = roblox::authenticated_user(&token).await?;

        let (id, existing) = {
            let s = self.read();
            if let Some(expected) = reauth.and_then(|id| s.bootstrapper.account(id)) {
                if expected.user_id != user.id {
                    return Err(format!(
                        "You signed in as @{}, but this profile belongs to @{}.",
                        user.username, expected.username
                    ));
                }
            }
            let existing = s.bootstrapper.accounts.iter().find(|a| a.user_id == user.id).map(|a| a.id);
            (existing.unwrap_or_else(Uuid::new_v4), existing.is_some())
        };

        let save = {
            let token = token.clone();
            tokio::task::spawn_blocking(move || credentials::store_session(id, &token))
        };
        save.await.map_err(|e| e.to_string())??;

        self.mutate(|s| {
            let now = s.now();
            match s.bootstrapper.account_mut(id) {
                Some(account) => {
                    account.username = user.username.clone();
                    account.display_name = user.display_name.clone();
                    account.needs_sign_in = false;
                }
                None => {
                    s.bootstrapper.accounts.push(Account {
                        id,
                        user_id: user.id,
                        username: user.username.clone(),
                        display_name: user.display_name.clone(),
                        alias: None,
                        added_at: now,
                        last_used: None,
                        needs_sign_in: false,
                        custom_settings: false,
                    });
                    if s.bootstrapper.preferences.default_account.is_none() {
                        s.bootstrapper.preferences.default_account = Some(id);
                    }
                    if s.bootstrapper.active_account.is_none() {
                        s.bootstrapper.active_account = Some(id);
                    }
                }
            }
            s.dirty = true;
        });
        self.toast(
            Tone::Positive,
            if existing {
                format!("Signed in as @{}", user.username)
            } else {
                format!("Added {} (@{})", user.display_name, user.username)
            },
        );
        self.spawn(|s| async move { s.ensure_artwork().await });
        Ok(user)
    }

    // ── Quick Login ──────────────────────────────────────────────────────

    /// Asks Roblox for a Quick Login code to show the user.
    pub async fn quick_login_start(&self) -> Result<String, String> {
        let login = roblox::quick_login_create().await?;
        let code = login.code.clone();
        self.read().quick_login = Some(login);
        Ok(code)
    }

    pub async fn quick_login_check(self: &Shared, reauth: Option<Uuid>) -> Result<QuickLoginUpdate, String> {
        let login = self.read().quick_login.clone().ok_or("Get a new code to continue.")?;
        let status = roblox::quick_login_status(login.clone()).await?;
        match status {
            QuickLoginStatus::Validated => {
                self.read().quick_login = None;
                let token = roblox::quick_login_complete(login).await?;
                self.add_session(token, reauth).await?;
                Ok(QuickLoginUpdate::Done)
            }
            QuickLoginStatus::Expired | QuickLoginStatus::Cancelled => {
                self.read().quick_login = None;
                Ok(QuickLoginUpdate::Expired { status })
            }
            status => Ok(QuickLoginUpdate::Waiting { status }),
        }
    }

    // ── Signing in on roblox.com ─────────────────────────────────────────

    /// Opens Roblox's own sign-in (or sign-up) page in a private browser
    /// window. Pious never sees what's typed there; once Roblox issues a
    /// session cookie, it's saved and the window closes.
    pub fn open_sign_in(self: &Shared, sign_up: bool, reauth: Option<Uuid>) -> Result<(), String> {
        let app = self.app().clone();
        if let Some(existing) = app.get_webview_window(SIGN_IN_WINDOW) {
            let _ = existing.set_focus();
            return Ok(());
        }
        let url = if sign_up { "https://www.roblox.com/signup" } else { "https://www.roblox.com/login" };
        let mut builder = WebviewWindowBuilder::new(&app, SIGN_IN_WINDOW, WebviewUrl::External(url.parse().unwrap()))
            .title(if sign_up { "Create a Roblox account" } else { "Sign in with Roblox" })
            .inner_size(1000.0, 760.0)
            .min_inner_size(480.0, 520.0)
            .center()
            .incognito(true)
            .data_directory(store::data_dir().join("WebView2"));
        if let Some(main) = app.get_webview_window("main") {
            builder = builder.parent(&main).map_err(|e| e.to_string())?;
        }
        builder.build().map_err(|e| format!("Couldn't open the sign-in window: {e}"))?;

        self.mutate(|s| s.sign_in = Some(SignIn { reauth, verifying: false }));
        self.spawn(move |s| async move { s.watch_sign_in().await });
        Ok(())
    }

    /// Waits for the sign-in window to receive a session cookie.
    async fn watch_sign_in(self: Shared) {
        let roblox: url::Url = "https://www.roblox.com/".parse().unwrap();
        loop {
            tokio::time::sleep(Duration::from_millis(900)).await;
            let Some(window) = self.app().get_webview_window(SIGN_IN_WINDOW) else {
                // Closed by the user.
                self.mutate(|s| s.sign_in = None);
                return;
            };
            let token = window.cookies_for_url(roblox.clone()).ok().and_then(|cookies| {
                cookies
                    .into_iter()
                    .find(|c| c.name() == ".ROBLOSECURITY" && !c.value().is_empty())
                    .map(|c| c.value().to_owned())
            });
            let Some(token) = token else { continue };

            let reauth = self.mutate(|s| {
                let sign_in = s.sign_in.as_mut()?;
                sign_in.verifying = true;
                Some(sign_in.reauth)
            });
            let Some(reauth) = reauth else { return };
            match self.add_session(token, reauth).await {
                Ok(_) => {
                    let _ = window.close();
                    self.mutate(|s| s.sign_in = None);
                    return;
                }
                Err(error) => {
                    self.toast(Tone::Negative, error);
                    let _ = window.clear_all_browsing_data();
                    self.mutate(|s| {
                        if let Some(sign_in) = &mut s.sign_in {
                            sign_in.verifying = false;
                        }
                    });
                }
            }
        }
    }

    pub fn close_sign_in(&self) {
        if let Some(window) = self.app().get_webview_window(SIGN_IN_WINDOW) {
            let _ = window.close();
        }
    }

    // ── Profiles ─────────────────────────────────────────────────────────

    pub fn remove_account(&self, id: Uuid) {
        credentials::delete_session(id);
        self.mutate(|s| {
            s.bootstrapper.accounts.retain(|a| a.id != id);
            if s.bootstrapper.preferences.default_account == Some(id) {
                s.bootstrapper.preferences.default_account = None;
            }
            if s.bootstrapper.active_account == Some(id) {
                s.bootstrapper.active_account = s.bootstrapper.default_account().map(|a| a.id);
            }
            for game in &mut s.bootstrapper.games {
                if game.config.account == Some(id) {
                    game.config.account = None;
                    game.config.pin_account = false;
                }
            }
            s.dirty = true;
        });
        self.toast(Tone::Neutral, "Account removed from Pious");
    }

    /// Gives an account its own Roblox settings (starting from the PC's),
    /// or goes back to sharing the PC's.
    pub fn set_custom_settings(&self, id: Uuid, on: bool) {
        let label = self.mutate(|s| {
            let account = s.bootstrapper.account_mut(id)?;
            account.custom_settings = on;
            s.dirty = true;
            Some(account.label().to_owned())
        });
        if let (Some(label), true) = (label, on) {
            self.toast(Tone::Positive, format!("{label} now keeps its own Roblox settings."));
        }
    }

    pub fn rename_account(&self, id: Uuid, alias: String) {
        let alias = alias.trim().to_owned();
        self.mutate(|s| {
            if let Some(account) = s.bootstrapper.account_mut(id) {
                account.alias = (!alias.is_empty() && alias != account.display_name).then_some(alias);
                s.dirty = true;
            }
        });
    }

    pub fn set_default_account(&self, id: Option<Uuid>) {
        let label = self.mutate(|s| {
            s.bootstrapper.preferences.default_account = id;
            s.dirty = true;
            id.and_then(|id| s.bootstrapper.account(id)).map(|a| a.label().to_owned())
        });
        if let Some(label) = label {
            self.toast(Tone::Positive, format!("{label} is now your default account"));
        }
    }

    /// Picks the account Play uses this session (the sidebar switcher).
    /// The default account doesn't change.
    pub fn set_active_account(self: &Shared, id: Uuid) {
        let label = self.mutate(|s| {
            let label = s.bootstrapper.account(id).map(|a| a.label().to_owned())?;
            s.bootstrapper.active_account = Some(id);
            Some(label)
        });
        if let Some(label) = label {
            self.toast(Tone::Positive, format!("Play now launches as {label}"));
        }
    }
}
