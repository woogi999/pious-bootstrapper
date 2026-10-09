//! Choosing where public servers are: Roblox's own matchmaking ("auto"),
//! the server with the best ping, or a region.
//!
//! Roblox's server list has no locations, so for a region Pious asks
//! Roblox's join API (as the account that's about to play, exactly like
//! the client would) which address a candidate server has, looks that up,
//! and joins the first server in the region. Only a handful of servers are
//! checked so it stays quick; with none in the region, the game starts with
//! Roblox's own choice and says so.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::core::roblox::{self, PublicServer};

/// Regions people can pick: ID, name.
pub const REGIONS: [(&str, &str); 9] = [
    ("us_east", "US East"),
    ("us_central", "US Central"),
    ("us_west", "US West"),
    ("europe", "Europe"),
    ("uk", "United Kingdom"),
    ("asia", "Asia"),
    ("oceania", "Australia and Oceania"),
    ("south_america", "South America"),
    ("india", "India"),
];

/// Servers checked for a region before giving up.
const CANDIDATES: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Located {
    pub region: Option<String>,
    /// "Ashburn, Virginia, US".
    pub label: String,
}

/// The region of a place, from its country and time zone (what IP lookups
/// say about a server).
pub fn classify(country: &str, timezone: &str) -> Option<&'static str> {
    let country = country.to_ascii_uppercase();
    Some(match (country.as_str(), timezone) {
        ("US" | "CA", tz) if tz.contains("New_York") || tz.contains("Toronto") || tz.contains("Detroit") || tz.contains("Indiana") || tz.contains("Kentucky") || tz.contains("Montreal") => "us_east",
        ("US" | "CA", tz) if tz.contains("Chicago") || tz.contains("Winnipeg") || tz.contains("Denver") || tz.contains("Edmonton") || tz.contains("Boise") => "us_central",
        ("US" | "CA", tz) if tz.contains("Los_Angeles") || tz.contains("Vancouver") || tz.contains("Phoenix") || tz.contains("Anchorage") || tz.contains("Honolulu") => "us_west",
        ("US", _) => "us_east",
        ("GB" | "IE", _) => "uk",
        ("IN", _) => "india",
        ("AU" | "NZ", _) => "oceania",
        ("BR" | "AR" | "CL" | "CO" | "PE" | "UY" | "PY" | "EC" | "VE" | "BO", _) => "south_america",
        (_, tz) if tz.starts_with("Europe/") => "europe",
        (_, tz) if tz.starts_with("Asia/") => "asia",
        (_, tz) if tz.starts_with("Australia/") || tz.starts_with("Pacific/") => "oceania",
        (_, tz) if tz.starts_with("America/") => "south_america",
        _ => return None,
    })
}

pub fn name(id: &str) -> &str {
    REGIONS.iter().find(|(r, _)| *r == id).map_or(id, |(_, n)| n)
}

/// Where an address is (cached for the session: servers don't move).
pub async fn locate(ip: &str) -> Result<Located, String> {
    static CACHE: Mutex<Option<HashMap<String, Located>>> = Mutex::new(None);
    if let Some(found) = CACHE.lock().ok().and_then(|c| c.as_ref()?.get(ip).cloned()) {
        return Ok(found);
    }
    #[derive(Deserialize)]
    struct Place {
        city: Option<String>,
        region: Option<String>,
        country: Option<String>,
        timezone: Option<String>,
    }
    let place: Place = roblox::client()
        .get(format!("https://ipinfo.io/{ip}/json"))
        .timeout(Duration::from_secs(6))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let country = place.country.clone().unwrap_or_default();
    let found = Located {
        region: classify(&country, place.timezone.as_deref().unwrap_or_default()).map(str::to_owned),
        label: [place.city, place.region, place.country].into_iter().flatten().filter(|p| !p.is_empty()).collect::<Vec<_>>().join(", "),
    };
    if let Ok(mut cache) = CACHE.lock() {
        cache.get_or_insert_with(HashMap::new).insert(ip.to_owned(), found.clone());
    }
    Ok(found)
}

/// The address of one public server, from Roblox's join API (what the
/// client asks when it joins). `None` if Roblox wouldn't say.
async fn server_address(token: &str, place_id: u64, job: &str) -> Option<String> {
    let cookie = roblox::session_cookie(token);
    let body = json!({ "placeId": place_id, "gameId": job, "isTeleport": false, "gameJoinAttemptId": uuid::Uuid::new_v4().to_string() });
    let response = roblox::send_with_csrf(|| {
        roblox::client()
            .post("https://gamejoin.roblox.com/v1/join-game-instance")
            .header("Cookie", &cookie)
            .header("User-Agent", "Roblox/WinInet")
            .timeout(Duration::from_secs(8))
            .json(&body)
    })
    .await
    .ok()?;
    let value: Value = response.json().await.ok()?;
    let script = &value["joinScript"];
    script["UdmuxEndpoints"][0]["Address"].as_str().or_else(|| script["MachineAddress"].as_str()).map(str::to_owned)
}

/// The server with the lowest ping that still has room.
pub fn best_ping(servers: &[PublicServer]) -> Option<&PublicServer> {
    servers
        .iter()
        .filter(|s| s.max_players == 0 || s.playing < s.max_players)
        .filter(|s| s.ping.is_some())
        .min_by_key(|s| (s.ping.unwrap_or(u32::MAX), std::cmp::Reverse(s.playing)))
}

/// Picks the server to join for `choice` ("best_ping" or a region ID), as
/// `token`'s account. `Err` says why none was picked (the game then starts
/// with Roblox's own choice).
pub async fn pick(choice: &str, token: &str, place_id: u64) -> Result<(String, String), String> {
    let servers = roblox::public_servers_paged(place_id, 1, Some(token)).await?;
    if servers.is_empty() {
        return Err("no public server has room".into());
    }
    if choice == "best_ping" {
        let server = best_ping(&servers).ok_or("Roblox didn't say any server's ping")?;
        return Ok((server.job.clone(), format!("the server with the best ping ({} ms)", server.ping.unwrap_or_default())));
    }
    if !REGIONS.iter().any(|(id, _)| *id == choice) {
        return Err(format!("{choice} isn't a region Pious knows"));
    }
    // Busier servers first (they're the ones people want to join), then
    // the lowest ping.
    let mut candidates: Vec<&PublicServer> = servers.iter().filter(|s| s.max_players == 0 || s.playing + 1 < s.max_players).collect();
    candidates.sort_by_key(|s| (std::cmp::Reverse(s.playing), s.ping.unwrap_or(u32::MAX)));
    for server in candidates.into_iter().take(CANDIDATES) {
        let Some(address) = server_address(token, place_id, &server.job).await else { continue };
        let Ok(located) = locate(&address).await else { continue };
        if located.region.as_deref() == Some(choice) {
            return Ok((server.job.clone(), format!("a server in {}", located.label)));
        }
    }
    Err(format!("none of the servers checked is in {}", name(choice)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_roblox_data_centers() {
        assert_eq!(classify("US", "America/New_York"), Some("us_east"));
        assert_eq!(classify("US", "America/Chicago"), Some("us_central"));
        assert_eq!(classify("US", "America/Los_Angeles"), Some("us_west"));
        assert_eq!(classify("NL", "Europe/Amsterdam"), Some("europe"));
        assert_eq!(classify("GB", "Europe/London"), Some("uk"));
        assert_eq!(classify("SG", "Asia/Singapore"), Some("asia"));
        assert_eq!(classify("AU", "Australia/Sydney"), Some("oceania"));
        assert_eq!(classify("BR", "America/Sao_Paulo"), Some("south_america"));
        assert_eq!(classify("IN", "Asia/Kolkata"), Some("india"));
        assert_eq!(classify("", ""), None);
    }

    #[test]
    fn best_ping_skips_full_and_unknown() {
        let server = |job: &str, playing, max, ping| PublicServer { job: job.into(), playing, max_players: max, ping, fps: None };
        let list = vec![server("full", 10, 10, Some(5)), server("unknown", 1, 10, None), server("far", 3, 10, Some(180)), server("near", 2, 10, Some(40))];
        assert_eq!(best_ping(&list).map(|s| s.job.as_str()), Some("near"));
        assert!(best_ping(&[]).is_none());
    }
}
