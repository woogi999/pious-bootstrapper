//! Thin client for the public and authenticated Roblox web APIs used by
//! Pious. All functions are async and return user-presentable errors.

use std::sync::OnceLock;
use std::time::Duration;

use reqwest::{Client, RequestBuilder, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Where a server is, from its public address: "Ashburn, Virginia, US".
pub async fn server_location(ip: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Place {
        city: Option<String>,
        region: Option<String>,
        country: Option<String>,
    }
    let place: Place = client()
        .get(format!("https://ipinfo.io/{ip}/json"))
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let parts: Vec<String> = [place.city, place.region, place.country].into_iter().flatten().filter(|p| !p.is_empty()).collect();
    if parts.is_empty() { Err("Unknown location".into()) } else { Ok(parts.join(", ")) }
}

pub(crate) fn client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent(concat!("Pious/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("HTTP client")
    })
}

pub(crate) fn net_err(e: reqwest::Error) -> String {
    if e.is_timeout() {
        "Roblox took too long to respond. Check your connection and try again.".into()
    } else if e.is_connect() {
        "Couldn't reach Roblox. Check your internet connection.".into()
    } else {
        format!("Network error: {e}")
    }
}

pub(crate) fn session_cookie(token: &str) -> String {
    format!(".ROBLOSECURITY={token}")
}

/// Sends a state-changing request, performing Roblox's CSRF token handshake.
pub(crate) async fn send_with_csrf(build: impl Fn() -> RequestBuilder) -> Result<Response, String> {
    let response = build().send().await.map_err(net_err)?;
    if response.status() == StatusCode::FORBIDDEN {
        if let Some(token) = response.headers().get("x-csrf-token").cloned() {
            return build()
                .header("x-csrf-token", token)
                .send()
                .await
                .map_err(net_err);
        }
    }
    Ok(response)
}

async fn get_json(url: &str) -> Result<Value, String> {
    let response = client().get(url).send().await.map_err(net_err)?;
    if !response.status().is_success() {
        return Err(format!("Roblox returned {} for {url}", response.status()));
    }
    response.json().await.map_err(net_err)
}

async fn get_bytes(url: &str) -> Result<Vec<u8>, String> {
    let response = client().get(url).send().await.map_err(net_err)?;
    if !response.status().is_success() {
        return Err(format!("Roblox returned {}", response.status()));
    }
    Ok(response.bytes().await.map_err(net_err)?.to_vec())
}

// ── Games ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInfo {
    pub place_id: u64,
    pub universe_id: u64,
    pub name: String,
    pub developer: Option<String>,
    pub description: Option<String>,
}

pub async fn fetch_game(place_id: u64) -> Result<GameInfo, String> {
    let universe = get_json(&format!(
        "https://apis.roblox.com/universes/v1/places/{place_id}/universe"
    ))
    .await
    .map_err(|_| format!("No Roblox experience was found for place {place_id}."))?;

    let universe_id = universe["universeId"]
        .as_u64()
        .ok_or_else(|| format!("No Roblox experience was found for place {place_id}."))?;

    let details = get_json(&format!(
        "https://games.roblox.com/v1/games?universeIds={universe_id}"
    ))
    .await?;
    let game = &details["data"][0];

    Ok(GameInfo {
        place_id,
        universe_id,
        name: game["name"].as_str().unwrap_or("Untitled experience").trim().to_owned(),
        developer: game["creator"]["name"].as_str().map(str::to_owned),
        description: game["description"]
            .as_str()
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .map(str::to_owned),
    })
}

/// Downloads the wide thumbnail for a game, falling back to its square icon.
pub async fn fetch_game_artwork(universe_id: u64) -> Result<Vec<u8>, String> {
    let thumbs = get_json(&format!(
        "https://thumbnails.roblox.com/v1/games/multiget/thumbnails?universeIds={universe_id}&countPerUniverse=1&defaults=true&size=768x432&format=Png&isCircular=false"
    ))
    .await?;

    let url = thumbs["data"][0]["thumbnails"][0]["imageUrl"].as_str().map(str::to_owned);
    let url = match url {
        Some(url) => url,
        None => {
            let icons = get_json(&format!(
                "https://thumbnails.roblox.com/v1/games/icons?universeIds={universe_id}&returnPolicy=PlaceHolder&size=512x512&format=Png&isCircular=false"
            ))
            .await?;
            icons["data"][0]["imageUrl"]
                .as_str()
                .ok_or("No artwork available")?
                .to_owned()
        }
    };
    get_bytes(&url).await
}

pub async fn fetch_avatar(user_id: u64) -> Result<Vec<u8>, String> {
    get_bytes(&avatar_url(user_id).await?).await
}

/// Where a user's avatar headshot is (a link others can load, e.g. Discord).
pub async fn avatar_url(user_id: u64) -> Result<String, String> {
    let thumbs = get_json(&format!(
        "https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds={user_id}&size=150x150&format=Png&isCircular=false"
    ))
    .await?;
    thumbs["data"][0]["imageUrl"].as_str().map(str::to_owned).ok_or_else(|| "No avatar available".to_owned())
}

// ── Accounts ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: u64,
    pub username: String,
    pub display_name: String,
}

/// Validates a session token and returns the user it belongs to.
pub async fn authenticated_user(token: &str) -> Result<UserInfo, String> {
    let response = client()
        .get("https://users.roblox.com/v1/users/authenticated")
        .header("Cookie", session_cookie(token))
        .send()
        .await
        .map_err(net_err)?;

    if response.status() == StatusCode::UNAUTHORIZED {
        return Err("That session is not valid or has expired. Please sign in again.".into());
    }
    if !response.status().is_success() {
        return Err(format!("Roblox returned {} while verifying the account.", response.status()));
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Me {
        id: u64,
        name: String,
        display_name: String,
    }

    let me: Me = response.json().await.map_err(net_err)?;
    Ok(UserInfo {
        id: me.id,
        username: me.name,
        display_name: me.display_name,
    })
}

/// A pending Roblox Quick Login session (the official cross-device sign-in).
#[derive(Debug, Clone)]
pub struct QuickLogin {
    pub code: String,
    pub private_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind")]
pub enum QuickLoginStatus {
    /// Waiting for the user to enter the code on a signed-in device.
    Pending,
    /// The user entered the code; waiting for them to confirm.
    Linked { account: Option<String> },
    /// Confirmed; the session can now be redeemed.
    Validated,
    Cancelled,
    Expired,
}

pub async fn quick_login_create() -> Result<QuickLogin, String> {
    let response = send_with_csrf(|| {
        client()
            .post("https://apis.roblox.com/auth-token-service/v1/login/create")
            .json(&json!({}))
    })
    .await?;
    if !response.status().is_success() {
        return Err(format!("Roblox couldn't start Quick Login ({}).", response.status()));
    }
    let body: Value = response.json().await.map_err(net_err)?;
    Ok(QuickLogin {
        code: body["code"].as_str().ok_or("Unexpected Quick Login response")?.to_owned(),
        private_key: body["privateKey"]
            .as_str()
            .ok_or("Unexpected Quick Login response")?
            .to_owned(),
    })
}

pub async fn quick_login_status(login: QuickLogin) -> Result<QuickLoginStatus, String> {
    let body = json!({ "code": login.code, "privateKey": login.private_key });
    let response = send_with_csrf(|| {
        client()
            .post("https://apis.roblox.com/auth-token-service/v1/login/status")
            .json(&body)
    })
    .await?;

    if response.status() == StatusCode::BAD_REQUEST {
        return Ok(QuickLoginStatus::Expired);
    }
    if !response.status().is_success() {
        return Err(format!("Roblox returned {} while checking Quick Login.", response.status()));
    }

    let body: Value = response.json().await.map_err(net_err)?;
    Ok(match body["status"].as_str().unwrap_or_default() {
        "Validated" => QuickLoginStatus::Validated,
        "UserLinked" => QuickLoginStatus::Linked {
            account: body["accountName"].as_str().map(str::to_owned),
        },
        "Cancelled" => QuickLoginStatus::Cancelled,
        _ => QuickLoginStatus::Pending,
    })
}

/// Redeems a validated Quick Login for a session token.
pub async fn quick_login_complete(login: QuickLogin) -> Result<String, String> {
    let body = json!({
        "ctype": "AuthToken",
        "cvalue": login.code,
        "password": login.private_key,
    });
    let response = send_with_csrf(|| {
        client()
            .post("https://auth.roblox.com/v2/login")
            .json(&body)
    })
    .await?;

    let token = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|cookie| {
            cookie
                .strip_prefix(".ROBLOSECURITY=")
                .map(|rest| rest.split(';').next().unwrap_or_default().to_owned())
        })
        .filter(|t| !t.is_empty());

    match token {
        Some(token) => Ok(token),
        None if response.headers().contains_key("rblx-challenge-id") => Err(
            "Roblox asked for an extra verification step that can't be completed here. \
             Try again, or use the session token option."
                .into(),
        ),
        None => Err(format!("Roblox didn't complete the sign-in ({}).", response.status())),
    }
}

// ── Players ──────────────────────────────────────────────────────────────

/// Looks up a player by username (or display-less user name).
pub async fn user_by_name(username: &str) -> Result<UserInfo, String> {
    let response = client()
        .post("https://users.roblox.com/v1/usernames/users")
        .json(&json!({ "usernames": [username.trim().trim_start_matches('@')], "excludeBannedUsers": true }))
        .send()
        .await
        .map_err(net_err)?;
    if !response.status().is_success() {
        return Err(format!("Roblox returned {} while looking up that player.", response.status()));
    }
    let body: Value = response.json().await.map_err(net_err)?;
    let user = &body["data"][0];
    match user["id"].as_u64() {
        Some(id) => Ok(UserInfo {
            id,
            username: user["name"].as_str().unwrap_or(username).to_owned(),
            display_name: user["displayName"].as_str().unwrap_or(username).to_owned(),
        }),
        None => Err(format!("There's no Roblox player called \"{}\".", username.trim())),
    }
}

/// Where a player is right now, as far as Roblox will tell us.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind")]
pub enum Presence {
    Offline,
    Online,
    InStudio,
    /// In an experience. `job` is only shown when the player lets anyone
    /// join them (or when they're your friend).
    InGame {
        place_id: Option<u64>,
        job: Option<String>,
        location: String,
    },
}

/// Tells Roblox the account is online on its app (shown to friends as
/// online, not in a game). Lasts a couple of minutes; send it again to stay.
pub async fn register_presence(token: &str) -> Result<(), String> {
    let cookie = session_cookie(token);
    let body = json!({ "location": "Home", "placeId": null, "disconnect": false });
    let response = send_with_csrf(|| {
        client()
            .post("https://presence.roblox.com/v1/presence/register-app-presence")
            .header("Cookie", &cookie)
            .json(&body)
    })
    .await?;
    if response.status().is_success() { Ok(()) } else { Err(format!("Roblox returned {}", response.status())) }
}

/// Reads a player's presence, signed in as one of the user's accounts.
/// The job ID comes back whenever the player's "who can join me" setting
/// allows the asking account, friend or not.
pub async fn presence(token: &str, user_id: u64) -> Result<Presence, String> {
    let cookie = session_cookie(token);
    let body = json!({ "userIds": [user_id] });
    let response = send_with_csrf(|| {
        client()
            .post("https://presence.roblox.com/v1/presence/users")
            .header("Cookie", &cookie)
            .json(&body)
    })
    .await?;
    if response.status() == StatusCode::UNAUTHORIZED {
        return Err("SESSION_EXPIRED".into());
    }
    if !response.status().is_success() {
        return Err(format!("Roblox returned {} while checking where they are.", response.status()));
    }
    let body: Value = response.json().await.map_err(net_err)?;
    let entry = &body["userPresences"][0];
    Ok(match entry["userPresenceType"].as_u64().unwrap_or(0) {
        1 => Presence::Online,
        2 => Presence::InGame {
            place_id: entry["placeId"].as_u64().or_else(|| entry["rootPlaceId"].as_u64()),
            job: entry["gameId"].as_str().filter(|g| !g.is_empty()).map(str::to_owned),
            location: entry["lastLocation"].as_str().unwrap_or_default().to_owned(),
        },
        3 => Presence::InStudio,
        _ => Presence::Offline,
    })
}

/// A public server of an experience.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicServer {
    pub job: String,
    pub playing: u32,
    pub max_players: u32,
    pub ping: Option<u32>,
    pub fps: Option<f32>,
}

/// Fetches a server-list page. Roblox rate-limits these hard (signed-out
/// requests get a 429 after the first one), so requests carry a session
/// when there is one and wait out rate limits a few times.
async fn server_page(url: &str, token: Option<&str>) -> Result<Value, String> {
    let mut wait = Duration::from_millis(1200);
    for attempt in 0..5 {
        let mut request = client().get(url);
        if let Some(token) = token {
            request = request.header("Cookie", session_cookie(token));
        }
        let response = request.send().await.map_err(net_err)?;
        let status = response.status();
        if status == StatusCode::TOO_MANY_REQUESTS && attempt < 4 {
            let after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .map(|s| Duration::from_secs(s.min(10)));
            tokio::time::sleep(after.unwrap_or(wait)).await;
            wait *= 2;
            continue;
        }
        if !status.is_success() {
            return Err(format!("Roblox returned {status}"));
        }
        return response.json().await.map_err(net_err);
    }
    Err("Roblox is limiting requests".into())
}

/// Public servers that still have room, up to `pages` pages of 100.
/// `token` is any signed-in session; Roblox allows far more requests then.
pub async fn public_servers_paged(place_id: u64, pages: usize, token: Option<&str>) -> Result<Vec<PublicServer>, String> {
    let mut servers = Vec::new();
    let mut cursor: Option<String> = None;
    for page in 0..pages.max(1) {
        let mut url = format!(
            "https://games.roblox.com/v1/games/{place_id}/servers/Public?sortOrder=Desc&excludeFullGames=true&limit=100"
        );
        if let Some(cursor) = &cursor {
            url.push_str(&format!("&cursor={cursor}"));
        }
        let body = match server_page(&url, token).await {
            Ok(body) => body,
            // Keep what we already have.
            Err(_) if page > 0 => break,
            Err(error) => {
                return Err(format!("Roblox didn't return the server list ({error}). Try again in a moment."));
            }
        };
        servers.extend(body["data"].as_array().into_iter().flatten().filter_map(|s| {
            Some(PublicServer {
                job: s["id"].as_str()?.to_owned(),
                playing: s["playing"].as_u64().unwrap_or(0) as u32,
                max_players: s["maxPlayers"].as_u64().unwrap_or(0) as u32,
                ping: s["ping"].as_u64().map(|p| p as u32),
                fps: s["fps"].as_f64().map(|f| f as f32),
            })
        }));
        cursor = body["nextPageCursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    servers.retain(|s| s.max_players == 0 || s.playing < s.max_players);
    Ok(servers)
}

/// One public server by its job ID, as Roblox's server list reports it
/// (players, and the server's own average ping and frame rate). Looks
/// through at most `pages` pages of 100, busiest first; `None` when it isn't
/// there (a private or reserved server, or too far down a huge game's list).
pub async fn find_public_server(place_id: u64, job: &str, pages: usize, token: Option<&str>) -> Result<Option<PublicServer>, String> {
    let mut cursor: Option<String> = None;
    for _ in 0..pages.max(1) {
        let mut url = format!("https://games.roblox.com/v1/games/{place_id}/servers/Public?sortOrder=Desc&limit=100");
        if let Some(cursor) = &cursor {
            url.push_str(&format!("&cursor={cursor}"));
        }
        let body = server_page(&url, token).await?;
        let found = body["data"].as_array().into_iter().flatten().find(|s| s["id"].as_str() == Some(job));
        if let Some(s) = found {
            return Ok(Some(PublicServer {
                job: job.to_owned(),
                playing: s["playing"].as_u64().unwrap_or(0) as u32,
                max_players: s["maxPlayers"].as_u64().unwrap_or(0) as u32,
                ping: s["ping"].as_u64().map(|p| p as u32),
                fps: s["fps"].as_f64().map(|f| f as f32),
            }));
        }
        cursor = body["nextPageCursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    Ok(None)
}

/// What a pop-up shows about a game: its name, icon, banner and how many
/// are playing. Links are Roblox's CDN (the window may load those).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GameCard {
    pub name: String,
    pub icon: Option<String>,
    pub banner: Option<String>,
    pub playing: Option<u64>,
    pub creator: Option<String>,
}

/// A game's card, by place (cached for the session).
pub async fn game_card(place_id: u64) -> Result<GameCard, String> {
    use std::collections::HashMap;
    use std::sync::Mutex;
    static CACHE: Mutex<Option<HashMap<u64, GameCard>>> = Mutex::new(None);
    if let Some(card) = CACHE.lock().ok().and_then(|c| c.as_ref()?.get(&place_id).cloned()) {
        return Ok(card);
    }
    let universe = get_json(&format!("https://apis.roblox.com/universes/v1/places/{place_id}/universe")).await?;
    let universe = universe["universeId"].as_u64().ok_or("No such game")?;
    let urls = [
        format!("https://games.roblox.com/v1/games?universeIds={universe}"),
        format!("https://thumbnails.roblox.com/v1/games/icons?universeIds={universe}&returnPolicy=PlaceHolder&size=150x150&format=Png&isCircular=false"),
        format!("https://thumbnails.roblox.com/v1/games/multiget/thumbnails?universeIds={universe}&countPerUniverse=1&defaults=true&size=768x432&format=Png&isCircular=false"),
    ];
    let (details, icon, banner) = futures::join!(get_json(&urls[0]), get_json(&urls[1]), get_json(&urls[2]));
    let details = details?;
    let game = &details["data"][0];
    let card = GameCard {
        name: game["name"].as_str().unwrap_or_default().trim().to_owned(),
        playing: game["playing"].as_u64(),
        creator: game["creator"]["name"].as_str().map(str::to_owned),
        icon: icon.ok().and_then(|v| v["data"][0]["imageUrl"].as_str().map(str::to_owned)),
        banner: banner.ok().and_then(|v| v["data"][0]["thumbnails"][0]["imageUrl"].as_str().map(str::to_owned)),
    };
    if let Ok(mut cache) = CACHE.lock() {
        cache.get_or_insert_with(HashMap::new).insert(place_id, card.clone());
    }
    Ok(card)
}

/// A web address for a game's square icon, for places that show pictures
/// by link (like Discord).
pub async fn game_icon_url(universe_id: u64) -> Result<String, String> {
    let icons = get_json(&format!(
        "https://thumbnails.roblox.com/v1/games/icons?universeIds={universe_id}&returnPolicy=PlaceHolder&size=512x512&format=Png&isCircular=false"
    ))
    .await?;
    icons["data"][0]["imageUrl"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "No icon available".into())
}

// ── Launching ────────────────────────────────────────────────────────────

/// Requests a one-time authentication ticket used to hand a session to the client.
pub async fn authentication_ticket(token: &str) -> Result<String, String> {
    let cookie = session_cookie(token);
    let response = send_with_csrf(|| {
        client()
            .post("https://auth.roblox.com/v1/authentication-ticket")
            .header("Cookie", &cookie)
            .header("Referer", "https://www.roblox.com/")
            .header("Content-Type", "application/json")
            .body("{}")
    })
    .await?;

    if response.status() == StatusCode::UNAUTHORIZED {
        return Err("SESSION_EXPIRED".into());
    }

    response
        .headers()
        .get("rbx-authentication-ticket")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .ok_or_else(|| format!("Roblox didn't issue a launch ticket ({}).", response.status()))
}

/// Resolves a `roblox.com/share?code=…&type=Server` link to its place and link code.
pub async fn resolve_share_link(token: &str, code: &str) -> Result<(u64, String), String> {
    let cookie = session_cookie(token);
    let body = json!({ "linkId": code, "linkType": "Server" });
    let response = send_with_csrf(|| {
        client()
            .post("https://apis.roblox.com/sharelinks/v1/resolve-link")
            .header("Cookie", &cookie)
            .json(&body)
    })
    .await?;
    if !response.status().is_success() {
        return Err("That private server link couldn't be opened. It may have expired.".into());
    }
    let body: Value = response.json().await.map_err(net_err)?;
    let invite = &body["privateServerInviteData"];
    match (invite["placeId"].as_u64(), invite["linkCode"].as_str()) {
        (Some(place), Some(link)) => Ok((place, link.to_owned())),
        _ => Err("That private server link is no longer valid.".into()),
    }
}

// ── Versions ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientVersion {
    pub version: String,
    pub hash: String,
}

impl From<crate::core::deployment::Deployment> for ClientVersion {
    fn from(d: crate::core::deployment::Deployment) -> Self {
        Self { version: d.version, hash: d.version_guid }
    }
}

/// A Windows player build listed by weao.xyz.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownBuild {
    /// "Current", "Previous", "Upcoming" or "History" (Roblox's deploy log).
    pub kind: String,
    pub version: String,
    pub hash: String,
    pub date: String,
}

/// The current, previous and upcoming Windows player builds from weao.xyz,
/// which keeps the build hashes Roblox no longer publishes itself.
pub async fn weao_builds() -> Result<Vec<KnownBuild>, String> {
    let mut builds = Vec::new();
    let mut last_error = None;
    for (path, kind) in [("current", "Current"), ("past", "Previous"), ("future", "Upcoming")] {
        let response = client()
            .get(format!("https://weao.xyz/api/versions/{path}"))
            // weao.xyz asks third-party tools to identify themselves this way.
            .header("User-Agent", "WEAO-3PService")
            .send()
            .await;
        let body: Value = match response {
            Ok(r) if r.status().is_success() => match r.json().await {
                Ok(body) => body,
                Err(e) => {
                    last_error = Some(net_err(e));
                    continue;
                }
            },
            Ok(r) => {
                last_error = Some(format!("weao.xyz returned {}", r.status()));
                continue;
            }
            Err(e) => {
                last_error = Some(net_err(e));
                continue;
            }
        };
        let Some(hash) = body["Windows"].as_str().filter(|h| h.starts_with("version-")) else {
            continue;
        };
        if builds.iter().any(|b: &KnownBuild| b.hash == hash) {
            continue;
        }
        builds.push(KnownBuild {
            kind: kind.to_owned(),
            version: body["WindowsResponse"]["version"].as_str().unwrap_or_default().to_owned(),
            hash: hash.to_owned(),
            date: body["WindowsDate"].as_str().unwrap_or_default().to_owned(),
        });
    }
    // Everything older, from Roblox's own deploy log.
    if let Ok(history) = deploy_history().await {
        for build in history {
            if !builds.iter().any(|b| b.hash == build.hash) {
                builds.push(build);
            }
        }
    }
    if builds.is_empty() {
        return Err(last_error.unwrap_or_else(|| "weao.xyz didn't list any Windows builds.".into()));
    }
    Ok(builds)
}

/// Windows player builds from Roblox's deploy log, newest first (at most
/// 300).
async fn deploy_history() -> Result<Vec<KnownBuild>, String> {
    let text = client()
        .get("https://setup.rbxcdn.com/DeployHistory.txt")
        .send()
        .await
        .map_err(net_err)?
        .text()
        .await
        .map_err(net_err)?;
    Ok(parse_deploy_history(&text, 300))
}

/// Lines as served (checked 2026-10-10), oldest first:
/// `New WindowsPlayer version-d599f7fc52a8404c at 3/3/2026 5:15:45 PM, file
/// version: 0, 711, 0, 7110875, git hash: 0.711.0.7110875 ...`; older ones
/// end `file version: 0, 641, 0, 6410693...Done!`. Since 3/17/2026 Roblox
/// writes `version-hidden` instead of the GUID; those lines are skipped.
fn parse_deploy_history(text: &str, limit: usize) -> Vec<KnownBuild> {
    let mut out = Vec::new();
    for line in text.lines().rev() {
        let Some(rest) = line.trim().strip_prefix("New WindowsPlayer ") else { continue };
        let Some((hash, rest)) = rest.split_once(" at ") else { continue };
        let Ok(hash) = crate::core::deployment::normalize_guid(hash) else { continue };
        if out.iter().any(|b: &KnownBuild| b.hash == hash) {
            continue;
        }
        let (date, version) = match rest.split_once(", file version: ") {
            Some((date, version)) => (date, version),
            None => (rest, ""),
        };
        let version = version.split("...").next().unwrap_or_default();
        let version = version.split(", git hash").next().unwrap_or_default().replace(' ', "").replace(',', ".");
        out.push(KnownBuild { kind: "History".into(), version, hash, date: date.trim().to_owned() });
        if out.len() >= limit {
            break;
        }
    }
    out
}

#[cfg(test)]
mod history_tests {
    #[test]
    fn reads_deploy_history() {
        let text = "New Studio64 version-aaaaaaaaaaaaaaaa at 1/1/2024 1:00:00 PM, file version: 0, 600, 0, 1...Done!
                    New WindowsPlayer version-bbbbbbbbbbbbbbbb at 9/4/2024 5:28:45 PM, file version: 0, 641, 0, 6410693...Done!
New WindowsPlayer version-d599f7fc52a8404c at 3/3/2026 5:15:45 PM, file version: 0, 711, 0, 7110875, git hash: 0.711.0.7110875 ...
New WindowsPlayer version-hidden at 3/17/2026 9:12:14 AM, file version: 0, 713, 0, 7130910, git hash: 0.713.0.7130910 ...
";
        let builds = super::parse_deploy_history(text, 10);
        assert_eq!(builds.len(), 2);
        assert_eq!(builds[0].hash, "version-d599f7fc52a8404c");
        assert_eq!(builds[0].version, "0.711.0.7110875");
        assert_eq!(builds[0].date, "3/3/2026 5:15:45 PM");
        assert_eq!(builds[1].hash, "version-bbbbbbbbbbbbbbbb");
        assert_eq!(builds[1].version, "0.641.0.6410693");
    }
}

// ── Discovery ────────────────────────────────────────────────────────────

/// A game suggested by Roblox's discovery services.
#[derive(Debug, Clone)]
pub struct SuggestedGame {
    pub universe_id: u64,
    pub place_id: u64,
    pub name: String,
    pub creator: String,
    pub players: u64,
    pub up_votes: u64,
    pub down_votes: u64,
}

/// Games Roblox considers similar to the given experience.
pub async fn similar_games(universe_id: u64) -> Result<Vec<SuggestedGame>, String> {
    let body = get_json(&format!(
        "https://games.roblox.com/v1/games/recommendations/game/{universe_id}?maxRows=30"
    ))
    .await?;
    Ok(body["games"]
        .as_array()
        .map(|games| {
            games
                .iter()
                .filter(|g| !g["isSponsored"].as_bool().unwrap_or(false))
                .filter_map(|g| {
                    Some(SuggestedGame {
                        universe_id: g["universeId"].as_u64()?,
                        place_id: g["placeId"].as_u64()?,
                        name: g["name"].as_str()?.trim().to_owned(),
                        creator: g["creatorName"].as_str().unwrap_or_default().to_owned(),
                        players: g["playerCount"].as_u64().unwrap_or(0),
                        up_votes: g["totalUpVotes"].as_u64().unwrap_or(0),
                        down_votes: g["totalDownVotes"].as_u64().unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}

/// Roblox's own charts (Trending, then Top Playing Now), as on its home
/// page, with no sign-in. Sponsored games are left out.
pub async fn popular_games() -> Result<Vec<SuggestedGame>, String> {
    let url = format!(
        "https://apis.roblox.com/explore-api/v1/get-sorts?sessionId={}&device=computer&country=all",
        uuid::Uuid::new_v4()
    );
    let body = get_json(&url).await?;
    let mut out: Vec<SuggestedGame> = Vec::new();
    for wanted in ["top-trending", "top-playing-now", "up-and-coming"] {
        let Some(sort) = body["sorts"].as_array().into_iter().flatten().find(|s| s["sortId"].as_str() == Some(wanted)) else { continue };
        for g in sort["games"].as_array().into_iter().flatten() {
            if g["isSponsored"].as_bool().unwrap_or(false) {
                continue;
            }
            let (Some(universe_id), Some(place_id), Some(name)) = (g["universeId"].as_u64(), g["rootPlaceId"].as_u64(), g["name"].as_str()) else { continue };
            if out.iter().any(|o| o.universe_id == universe_id) {
                continue;
            }
            out.push(SuggestedGame {
                universe_id,
                place_id,
                name: name.trim().to_owned(),
                creator: g["creatorName"].as_str().unwrap_or_default().to_owned(),
                players: g["playerCount"].as_u64().unwrap_or(0),
                up_votes: g["totalUpVotes"].as_u64().unwrap_or(0),
                down_votes: g["totalDownVotes"].as_u64().unwrap_or(0),
            });
        }
    }
    if out.is_empty() {
        return Err("Roblox didn't list any popular games.".into());
    }
    Ok(out)
}

/// A game found by name, with its icon (an image URL) when Roblox has one.
#[derive(Debug, Clone, Serialize)]
pub struct FoundGame {
    pub universe_id: u64,
    pub place_id: u64,
    pub name: String,
    pub creator: String,
    pub description: Option<String>,
    pub players: u64,
    pub up_votes: u64,
    pub down_votes: u64,
    pub icon: Option<String>,
}

/// Searches Roblox's experiences by name, like the search box on roblox.com
/// (no sign-in needed). Sponsored results are left out.
pub async fn search_games(query: &str) -> Result<Vec<FoundGame>, String> {
    let url = reqwest::Url::parse_with_params(
        "https://apis.roblox.com/search-api/omni-search",
        &[("searchQuery", query.trim()), ("pageType", "all"), ("sessionId", &uuid::Uuid::new_v4().to_string())],
    )
    .map_err(|e| e.to_string())?;
    let body = get_json(url.as_str()).await.map_err(|e| format!("Roblox's search didn't answer ({e})."))?;
    let mut found: Vec<FoundGame> = parse_search(&body);
    found.truncate(30);
    // Icons, all at once.
    if !found.is_empty() {
        let ids: Vec<String> = found.iter().map(|g| g.universe_id.to_string()).collect();
        let icons = get_json(&format!(
            "https://thumbnails.roblox.com/v1/games/icons?universeIds={}&size=150x150&format=Png&isCircular=false",
            ids.join(",")
        ))
        .await
        .unwrap_or(Value::Null);
        for entry in icons["data"].as_array().into_iter().flatten() {
            let (Some(id), Some(url)) = (entry["targetId"].as_u64(), entry["imageUrl"].as_str()) else { continue };
            if let Some(game) = found.iter_mut().find(|g| g.universe_id == id) {
                game.icon = Some(url.to_owned());
            }
        }
    }
    Ok(found)
}

fn parse_search(body: &Value) -> Vec<FoundGame> {
    let mut out: Vec<FoundGame> = Vec::new();
    for group in body["searchResults"].as_array().into_iter().flatten() {
        if group["contentGroupType"].as_str() != Some("Game") {
            continue;
        }
        for g in group["contents"].as_array().into_iter().flatten() {
            if g["isSponsored"].as_bool().unwrap_or(false) {
                continue;
            }
            let (Some(universe_id), Some(place_id), Some(name)) = (g["universeId"].as_u64(), g["rootPlaceId"].as_u64(), g["name"].as_str()) else {
                continue;
            };
            if out.iter().any(|f| f.universe_id == universe_id) {
                continue;
            }
            out.push(FoundGame {
                universe_id,
                place_id,
                name: name.trim().to_owned(),
                creator: g["creatorName"].as_str().unwrap_or_default().to_owned(),
                description: g["description"].as_str().map(str::trim).filter(|d| !d.is_empty()).map(str::to_owned),
                players: g["playerCount"].as_u64().unwrap_or(0),
                up_votes: g["totalUpVotes"].as_u64().unwrap_or(0),
                down_votes: g["totalDownVotes"].as_u64().unwrap_or(0),
                icon: None,
            });
        }
    }
    out
}

#[cfg(test)]
mod search_tests {
    #[test]
    fn reads_search_results() {
        let body = serde_json::json!({ "searchResults": [
            { "contentGroupType": "Game", "contents": [
                { "universeId": 1, "rootPlaceId": 10, "name": " Jailbreak ", "creatorName": "Badimo", "playerCount": 5, "totalUpVotes": 9, "totalDownVotes": 1, "isSponsored": false },
                { "universeId": 2, "rootPlaceId": 20, "name": "Ad", "isSponsored": true },
                { "universeId": 1, "rootPlaceId": 10, "name": "Jailbreak" }
            ]},
            { "contentGroupType": "User", "contents": [{ "universeId": 3, "rootPlaceId": 30, "name": "Not a game" }] }
        ]});
        let found = super::parse_search(&body);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Jailbreak");
        assert_eq!(found[0].place_id, 10);
    }
}

/// The experiences an account recently played on Roblox (from its home
/// feed's "Continue" row). Best effort: returns an empty list when Roblox's
/// response doesn't have the expected shape.
pub async fn recently_played(token: &str) -> Result<Vec<(u64, String)>, String> {
    let cookie = session_cookie(token);
    let body = json!({
        "pageType": "Home",
        "sessionId": uuid::Uuid::new_v4().to_string(),
        "supportedTreatmentTypes": ["SortlessGrid"],
    });
    let response = send_with_csrf(|| {
        client()
            .post("https://apis.roblox.com/discovery-api/omni-recommendation")
            .header("Cookie", &cookie)
            .json(&body)
    })
    .await?;
    if !response.status().is_success() {
        return Err(format!("Roblox returned {} for the home feed.", response.status()));
    }
    let body: Value = response.json().await.map_err(net_err)?;
    let names = &body["contentMetadata"]["Game"];

    let mut played = Vec::new();
    for sort in body["sorts"].as_array().into_iter().flatten() {
        let topic = sort["topic"].as_str().unwrap_or_default().to_ascii_lowercase();
        if !(topic.contains("continue") || topic.contains("recently")) {
            continue;
        }
        for item in sort["recommendationList"].as_array().into_iter().flatten() {
            if item["contentType"].as_str() != Some("Game") {
                continue;
            }
            if let Some(id) = item["contentId"].as_u64() {
                let name = names[id.to_string()]["name"].as_str().unwrap_or_default().to_owned();
                played.push((id, name));
            }
        }
    }
    Ok(played)
}

#[cfg(test)]
mod live_tests {
    /// Downloads real pictures from Roblox (needs the internet):
    /// `cargo test pictures_download -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn pictures_download() {
        let avatar = super::fetch_avatar(1).await;
        println!("avatar: {:?}", avatar.as_ref().map(Vec::len));
        let game = super::fetch_game_artwork(3508322461).await;
        println!("game: {:?}", game.as_ref().map(Vec::len));
        assert!(avatar.is_ok() && game.is_ok());
    }
}

pub(crate) fn http() -> &'static Client {
    client()
}

pub(crate) fn network_error(e: reqwest::Error) -> String {
    net_err(e)
}
