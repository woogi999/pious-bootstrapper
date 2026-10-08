//! Friends, what they're doing, and chatting with them, through Roblox's
//! own web APIs, signed in as one of the user's accounts. Also looks up
//! private servers' names.

use std::collections::HashMap;

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::roblox::{client, net_err, send_with_csrf, session_cookie};

/// One friend and what they're up to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Friend {
    pub id: u64,
    pub username: String,
    pub display_name: String,
    pub avatar: Option<String>,
    pub status: FriendStatus,
    /// The experience they're in, when Roblox shares it.
    pub location: Option<String>,
    pub place_id: Option<u64>,
    pub universe_id: Option<u64>,
    /// Their server, when they let you join them.
    pub job: Option<String>,
    pub last_online: Option<String>,
    /// Which of your accounts they're friends with (in the list of every
    /// account's friends); joining and chatting use the first.
    #[serde(default)]
    pub via: Vec<uuid::Uuid>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum FriendStatus {
    InGame,
    InStudio,
    Online,
    Offline,
}

async fn get_signed(url: &str, token: &str) -> Result<Value, String> {
    let response = client()
        .get(url)
        .header("Cookie", session_cookie(token))
        .send()
        .await
        .map_err(net_err)?;
    if response.status() == StatusCode::UNAUTHORIZED {
        return Err("SESSION_EXPIRED".into());
    }
    if !response.status().is_success() {
        return Err(format!("Roblox returned {}", response.status()));
    }
    response.json().await.map_err(net_err)
}

async fn post_signed(url: &str, token: &str, body: Value) -> Result<Value, String> {
    let cookie = session_cookie(token);
    let response = send_with_csrf(|| client().post(url).header("Cookie", &cookie).json(&body)).await?;
    if response.status() == StatusCode::UNAUTHORIZED {
        return Err("SESSION_EXPIRED".into());
    }
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Roblox returned {status} {}", text.chars().take(160).collect::<String>()));
    }
    let text = response.text().await.map_err(net_err)?;
    Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
}

/// Everyone on `user_id`'s friends list, with names, pictures and presence.
pub async fn friends(token: &str, user_id: u64) -> Result<Vec<Friend>, String> {
    // 1. Their IDs (Roblox pages these).
    let mut ids = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..20 {
        let mut url = format!("https://friends.roblox.com/v1/users/{user_id}/friends/find?limit=50&userSort=1");
        if let Some(c) = &cursor {
            url.push_str(&format!("&cursor={}", urlencode(c)));
        }
        let page = get_signed(&url, token).await?;
        ids.extend(page["PageItems"].as_array().into_iter().flatten().filter_map(|i| i["id"].as_u64()));
        cursor = page["NextCursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    // 2. Names, presence and pictures, in batches.
    let mut names: HashMap<u64, (String, String)> = HashMap::new();
    let mut presence: HashMap<u64, Value> = HashMap::new();
    let mut avatars: HashMap<u64, String> = HashMap::new();
    for chunk in ids.chunks(100) {
        let users = post_signed(
            "https://users.roblox.com/v1/users",
            token,
            json!({ "userIds": chunk, "excludeBannedUsers": false }),
        )
        .await?;
        for u in users["data"].as_array().into_iter().flatten() {
            if let Some(id) = u["id"].as_u64() {
                names.insert(
                    id,
                    (u["name"].as_str().unwrap_or_default().to_owned(), u["displayName"].as_str().unwrap_or_default().to_owned()),
                );
            }
        }
        if let Ok(p) = post_signed("https://presence.roblox.com/v1/presence/users", token, json!({ "userIds": chunk })).await {
            for entry in p["userPresences"].as_array().into_iter().flatten() {
                if let Some(id) = entry["userId"].as_u64() {
                    presence.insert(id, entry.clone());
                }
            }
        }
        let list = chunk.iter().map(u64::to_string).collect::<Vec<_>>().join(",");
        if let Ok(t) = get_signed(
            &format!("https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds={list}&size=48x48&format=Png&isCircular=false"),
            token,
        )
        .await
        {
            for entry in t["data"].as_array().into_iter().flatten() {
                if let (Some(id), Some(url)) = (entry["targetId"].as_u64(), entry["imageUrl"].as_str()) {
                    avatars.insert(id, url.to_owned());
                }
            }
        }
    }

    let mut out: Vec<Friend> = ids
        .into_iter()
        .map(|id| {
            let (username, display_name) = names.remove(&id).unwrap_or_default();
            let p = presence.get(&id).cloned().unwrap_or(Value::Null);
            let status = match p["userPresenceType"].as_u64().unwrap_or(0) {
                1 => FriendStatus::Online,
                2 => FriendStatus::InGame,
                3 => FriendStatus::InStudio,
                _ => FriendStatus::Offline,
            };
            let text = |key: &str| p[key].as_str().filter(|s| !s.is_empty()).map(str::to_owned);
            Friend {
                id,
                username,
                display_name,
                avatar: avatars.remove(&id),
                status,
                location: text("lastLocation").filter(|l| status == FriendStatus::InGame || l != "Website"),
                place_id: p["placeId"].as_u64().or_else(|| p["rootPlaceId"].as_u64()),
                universe_id: p["universeId"].as_u64(),
                job: text("gameId"),
                last_online: text("lastOnline"),
                via: Vec::new(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.status.cmp(&b.status).then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase())));
    Ok(out)
}

fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

// ── Notifications ────────────────────────────────────────────────────────

/// Someone who wants to be friends.
#[derive(Debug, Clone, PartialEq)]
pub struct FriendRequest {
    pub id: u64,
    pub username: String,
    pub display_name: String,
    pub avatar: Option<String>,
}

/// The newest friend requests.
pub async fn friend_requests(token: &str) -> Result<Vec<FriendRequest>, String> {
    let body = get_signed("https://friends.roblox.com/v1/my/friends/requests?limit=10&sortOrder=Desc", token).await?;
    Ok(body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let id = r["id"].as_u64()?;
            let username = r["name"].as_str().unwrap_or_default().to_owned();
            let display_name = r["displayName"].as_str().filter(|n| !n.is_empty()).unwrap_or(&username).to_owned();
            Some(FriendRequest { id, username, display_name, avatar: None })
        })
        .collect())
}

/// Pictures of a few players, by user ID.
pub async fn headshots(token: &str, ids: &[u64]) -> HashMap<u64, String> {
    let mut out = HashMap::new();
    if ids.is_empty() {
        return out;
    }
    let list = ids.iter().map(u64::to_string).collect::<Vec<_>>().join(",");
    if let Ok(t) = get_signed(
        &format!("https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds={list}&size=48x48&format=Png&isCircular=false"),
        token,
    )
    .await
    {
        for entry in t["data"].as_array().into_iter().flatten() {
            if let (Some(id), Some(url)) = (entry["targetId"].as_u64(), entry["imageUrl"].as_str()) {
                out.insert(id, url.to_owned());
            }
        }
    }
    out
}

/// How many of Roblox's own notifications (the bell) are unread.
pub async fn unread_notifications(token: &str) -> Result<u64, String> {
    let body = get_signed("https://notificationstream.roblox.com/v2/stream-notifications/unread-count", token).await?;
    Ok(body["unreadNotifications"].as_u64().unwrap_or(0))
}

// ── Chat ─────────────────────────────────────────────────────────────────

const CHAT: &str = "https://apis.roblox.com/platform-chat-api/v1";

/// A chat conversation (one-to-one or group).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Conversation {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub participants: Vec<u64>,
    pub unread: bool,
    pub updated: Option<String>,
    pub preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub id: String,
    pub sender: u64,
    pub content: String,
    pub sent: Option<String>,
}

fn str_of(v: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| match &v[*k] {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
}

fn u64_of(v: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|k| v[*k].as_u64().or_else(|| v[*k].as_str().and_then(|s| s.parse().ok())))
}

fn conversation(v: &Value) -> Option<Conversation> {
    let id = str_of(v, &["id", "conversation_id"])?;
    let participants = v["participant_user_ids"]
        .as_array()
        .or_else(|| v["participantUserIds"].as_array())
        .map(|a| a.iter().filter_map(|x| x.as_u64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))).collect())
        .unwrap_or_default();
    let last = v["messages"].as_array().and_then(|m| m.first()).cloned().unwrap_or(Value::Null);
    Some(Conversation {
        id,
        kind: str_of(v, &["type", "conversation_type"]).unwrap_or_else(|| "one_to_one".into()),
        name: str_of(v, &["name", "title"]).unwrap_or_default(),
        participants,
        unread: v["unread_message_count"].as_u64().unwrap_or(0) > 0 || v["has_unread_messages"].as_bool().unwrap_or(false),
        updated: str_of(v, &["updated_at", "last_updated", "created_at"]),
        preview: str_of(&last, &["content", "text"]),
    })
}

fn message(v: &Value) -> Option<Message> {
    Some(Message {
        id: str_of(v, &["id", "message_id"])?,
        sender: u64_of(v, &["sender_user_id", "senderTargetId", "sender_id"]).unwrap_or(0),
        content: str_of(v, &["content", "text"]).unwrap_or_default(),
        sent: str_of(v, &["created_at", "sent"]),
    })
}

/// The account's recent conversations, newest first.
pub async fn conversations(token: &str) -> Result<Vec<Conversation>, String> {
    let body = get_signed(&format!("{CHAT}/get-user-conversations?include_user_data=true&pageSize=50"), token).await?;
    Ok(body["conversations"].as_array().into_iter().flatten().filter_map(conversation).collect())
}

/// A conversation's latest messages, oldest first.
pub async fn messages(token: &str, conversation_id: &str) -> Result<Vec<Message>, String> {
    let body = get_signed(
        &format!("{CHAT}/get-conversation-messages?conversation_id={}&pageSize=50", urlencode(conversation_id)),
        token,
    )
    .await?;
    let mut list: Vec<Message> = body["messages"].as_array().into_iter().flatten().filter_map(message).collect();
    list.reverse();
    Ok(list)
}

/// Opens (or finds) the one-to-one conversation with a friend.
pub async fn conversation_with(token: &str, user_id: u64) -> Result<String, String> {
    let body = post_signed(
        &format!("{CHAT}/create-conversations"),
        token,
        json!({ "conversations": [{ "type": "one_to_one", "participant_user_ids": [user_id] }], "include_user_data": true }),
    )
    .await?;
    body["conversations"]
        .as_array()
        .and_then(|c| c.first())
        .and_then(|c| str_of(c, &["id", "conversation_id"]))
        .ok_or_else(|| "Roblox didn't open a chat with them. You can only message friends.".into())
}

pub async fn send(token: &str, conversation_id: &str, text: &str) -> Result<(), String> {
    post_signed(
        &format!("{CHAT}/send-messages"),
        token,
        json!({ "conversation_id": conversation_id, "messages": [{ "content": text }] }),
    )
    .await
    .map(|_| ())
}

// ── Private servers ──────────────────────────────────────────────────────

/// What a share link points to.
#[derive(Debug, Clone)]
pub struct ShareInvite {
    pub place_id: u64,
    pub private_server_id: Option<u64>,
    pub owner_id: Option<u64>,
}

pub async fn resolve_share(token: &str, code: &str) -> Result<ShareInvite, String> {
    let body = post_signed(
        "https://apis.roblox.com/sharelinks/v1/resolve-link",
        token,
        json!({ "linkId": code, "linkType": "Server" }),
    )
    .await
    .map_err(|_| "That private server link couldn't be opened. It may have expired.".to_owned())?;
    let invite = &body["privateServerInviteData"];
    match (invite["placeId"].as_u64(), invite["linkCode"].as_str()) {
        (Some(place_id), Some(_)) => Ok(ShareInvite {
            place_id,
            private_server_id: invite["privateServerId"].as_u64(),
            owner_id: invite["ownerUserId"].as_u64(),
        }),
        _ => Err("That private server link is no longer valid.".into()),
    }
}

/// A private server's own name, when the account can see it.
pub async fn private_server_name(token: &str, place_id: u64, server_id: Option<u64>, owner_id: Option<u64>) -> Option<String> {
    if let Some(id) = server_id {
        if let Ok(list) = get_signed(&format!("https://games.roblox.com/v1/games/{place_id}/private-servers?limit=100"), token).await {
            if let Some(found) = list["data"].as_array().into_iter().flatten().find(|s| s["vipServerId"].as_u64() == Some(id)) {
                if let Some(name) = found["name"].as_str().filter(|n| !n.trim().is_empty()) {
                    return Some(name.trim().to_owned());
                }
            }
        }
        if let Ok(server) = get_signed(&format!("https://games.roblox.com/v1/vip-servers/{id}"), token).await {
            if let Some(name) = server["name"].as_str().filter(|n| !n.trim().is_empty()) {
                return Some(name.trim().to_owned());
            }
        }
    }
    // Fall back to whose server it is.
    let owner = owner_id?;
    let user = get_signed(&format!("https://users.roblox.com/v1/users/{owner}"), token).await.ok()?;
    let name = user["displayName"].as_str().or_else(|| user["name"].as_str())?;
    Some(format!("{name}'s server"))
}
