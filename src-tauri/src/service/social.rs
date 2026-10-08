//! Friends and chat, for one of the user's accounts at a time.

use std::time::{Duration, Instant};

use serde::Serialize;
use uuid::Uuid;

use super::{Service, Shared};
use crate::core::credentials;
use crate::core::social::{self, Conversation, Friend, Message};

/// The friends list being shown.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Friends {
    pub account: Option<Uuid>,
    /// The list holds the friends of every account, not just `account`'s.
    pub all: bool,
    pub list: Vec<Friend>,
    pub loading: bool,
    pub error: Option<String>,
    #[serde(skip)]
    pub fetched: Option<Instant>,
}

fn friendly(error: String) -> String {
    if error == "SESSION_EXPIRED" {
        "That account's session expired. Sign it in again in Accounts.".into()
    } else if error.contains("429") {
        "Roblox is limiting requests for a moment. Pious tries again shortly.".into()
    } else {
        error
    }
}

impl Service {
    async fn session_of(&self, account: Uuid) -> Result<(String, u64), String> {
        let user_id = self.read().bootstrapper.account(account).map(|a| a.user_id).ok_or("That account isn't in Pious anymore.")?;
        let token = tokio::task::spawn_blocking(move || credentials::load_session(account))
            .await
            .map_err(|e| e.to_string())??;
        Ok((token, user_id))
    }

    /// Loads (or reloads) an account's friends; the play-as account's when
    /// `account` is `None`.
    pub async fn refresh_friends(self: &Shared, account: Option<Uuid>) {
        let (all, accounts) = {
            let s = self.read();
            let ids: Vec<Uuid> = s.bootstrapper.accounts.iter().filter(|a| !a.needs_sign_in).map(|a| a.id).collect();
            (s.bootstrapper.preferences.friends_all && ids.len() > 1, ids)
        };
        if all {
            return self.refresh_all_friends(accounts).await;
        }
        let Some(account) = account.or_else(|| self.read().bootstrapper.active_account) else { return };
        self.mutate(|s| {
            if s.friends.account != Some(account) || s.friends.all {
                s.friends.all = false;
                // What this account's list looked like last time, until it loads.
                s.friends.list = crate::core::cache::load(&format!("friends-{account}")).unwrap_or_default();
                s.friends.error = None;
            }
            s.friends.account = Some(account);
            s.friends.loading = true;
        });
        let result = match self.session_of(account).await {
            Ok((token, user)) => social::friends(&token, user).await.map_err(friendly),
            Err(error) => Err(error),
        };
        let fetched = result.is_ok();
        self.mutate(|s| {
            // Another account (or every account) may have been picked meanwhile.
            if s.friends.account != Some(account) || s.friends.all {
                return;
            }
            s.friends.loading = false;
            match result {
                Ok(list) => {
                    crate::core::cache::save(&format!("friends-{account}"), &list);
                    s.friends.list = list;
                    s.friends.error = None;
                    s.friends.fetched = Some(Instant::now());
                }
                Err(error) => s.friends.error = Some(error),
            }
        });
        // Pictures of the games friends are playing.
        if fetched {
            self.ensure_artwork().await;
        }
    }

    /// Every account's friends in one list. Someone who's friends with more
    /// than one of your accounts shows once, with the accounts they know.
    async fn refresh_all_friends(self: &Shared, accounts: Vec<Uuid>) {
        self.mutate(|s| {
            if !s.friends.all {
                // Last time's list, else the one on screen, until it loads.
                let cached: Vec<Friend> = crate::core::cache::load("friends-all").unwrap_or_default();
                if !cached.is_empty() {
                    s.friends.list = cached;
                } else if let Some(account) = s.friends.account {
                    for friend in &mut s.friends.list {
                        friend.via = vec![account];
                    }
                }
                s.friends.error = None;
            }
            s.friends.all = true;
            s.friends.account = s.bootstrapper.active_account.or(accounts.first().copied());
            s.friends.loading = true;
        });
        // One account after another: all at once trips Roblox's rate limit.
        let mut lists = Vec::new();
        for &account in &accounts {
            let result = match self.session_of(account).await {
                Ok((token, user)) => social::friends(&token, user).await.map_err(friendly),
                Err(error) => Err(error),
            };
            lists.push((account, result));
        }
        let mut merged: Vec<Friend> = Vec::new();
        let mut errors = Vec::new();
        for (account, result) in lists {
            match result {
                Ok(list) => {
                    for mut friend in list {
                        match merged.iter_mut().find(|f| f.id == friend.id) {
                            Some(known) => known.via.push(account),
                            None => {
                                friend.via = vec![account];
                                merged.push(friend);
                            }
                        }
                    }
                }
                Err(error) => errors.push(error),
            }
        }
        merged.sort_by(|a, b| a.status.cmp(&b.status).then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase())));
        let fetched = errors.len() < accounts.len();
        self.mutate(|s| {
            if !s.friends.all {
                return;
            }
            s.friends.loading = false;
            if fetched {
                crate::core::cache::save("friends-all", &merged);
                s.friends.list = merged;
                s.friends.fetched = Some(Instant::now());
            }
            s.friends.error = errors.into_iter().next();
        });
        if fetched {
            self.ensure_artwork().await;
        }
    }

    /// Keeps the friends list fresh while it's been looked at recently.
    pub(super) async fn friends_loop(self: Shared) {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let stale = {
                let s = self.read();
                s.friends.account.is_some() && !s.friends.loading && s.friends.fetched.is_some_and(|t| t.elapsed() > Duration::from_secs(25))
            };
            if stale {
                let account = self.read().friends.account;
                self.refresh_friends(account).await;
            }
        }
    }

    pub async fn conversations(&self, account: Uuid) -> Result<Vec<Conversation>, String> {
        let (token, _) = self.session_of(account).await?;
        let list = social::conversations(&token).await.map_err(friendly)?;
        crate::core::cache::save(&format!("conversations-{account}"), &list);
        Ok(list)
    }

    pub async fn chat_messages(&self, account: Uuid, conversation: String) -> Result<Vec<Message>, String> {
        let (token, _) = self.session_of(account).await?;
        let messages = social::messages(&token, &conversation).await.map_err(friendly)?;
        crate::core::cache::save(&format!("chat-{account}-{conversation}"), &messages);
        Ok(messages)
    }

    /// The messages of a chat as they were last seen (shown while fresh
    /// ones load).
    pub fn cached_messages(&self, account: Uuid, conversation: &str) -> Vec<Message> {
        crate::core::cache::load(&format!("chat-{account}-{conversation}")).unwrap_or_default()
    }

    pub fn cached_conversations(&self, account: Uuid) -> Vec<Conversation> {
        crate::core::cache::load(&format!("conversations-{account}")).unwrap_or_default()
    }

    pub async fn open_chat(&self, account: Uuid, user: u64) -> Result<String, String> {
        let (token, _) = self.session_of(account).await?;
        social::conversation_with(&token, user).await.map_err(friendly)
    }

    pub async fn send_chat(&self, account: Uuid, conversation: String, text: String) -> Result<(), String> {
        let text = text.trim().to_owned();
        if text.is_empty() {
            return Ok(());
        }
        let (token, _) = self.session_of(account).await?;
        social::send(&token, &conversation, &text).await.map_err(friendly)?;
        crate::core::stats::record("message", serde_json::json!({ "characters": text.chars().count() }));
        Ok(())
    }

    /// A private server link's game and (when it can be found) name.
    pub async fn server_link_info(&self, link: String) -> Result<ServerLinkInfo, String> {
        use crate::core::model::{ServerLink, parse_server_link};
        let parsed = parse_server_link(&link).ok_or("That isn't a private server link.")?;
        let token = self.any_session(None).await;
        let (place, server_id, owner) = match parsed {
            ServerLink::LinkCode { place_id, .. } => (place_id, None, None),
            ServerLink::Share { code } => {
                let token = token.as_deref().ok_or("Add an account first: Roblox only opens share links for signed-in players.")?;
                let invite = social::resolve_share(token, &code).await?;
                (Some(invite.place_id), invite.private_server_id, invite.owner_id)
            }
        };
        let name = match (token.as_deref(), place) {
            (Some(token), Some(place)) => social::private_server_name(token, place, server_id, owner).await,
            _ => None,
        };
        let game = match place {
            Some(place) => crate::core::roblox::fetch_game(place).await.ok().map(|g| g.name),
            None => None,
        };
        Ok(ServerLinkInfo { place_id: place, name, game })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerLinkInfo {
    pub place_id: Option<u64>,
    pub name: Option<String>,
    pub game: Option<String>,
}
