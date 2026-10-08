//! Turns a resolved launch plan into a running Roblox client.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use uuid::Uuid;

use crate::core::model::ServerLink;
use crate::core::{credentials, process, roblox};

/// Everything needed to start one client, already resolved from the library.
#[derive(Debug, Clone)]
pub struct LaunchRequest {
    pub account: Uuid,
    pub place_id: u64,
    pub server: Option<ServerLink>,
    /// A specific public server (its job ID) to join, e.g. a player's
    /// server or a server hop. Ignored when `server` is set.
    pub job: Option<String>,
    /// The specific client build to run. `None` hands off to the system's
    /// registered Roblox protocol handler.
    pub executable: Option<PathBuf>,
    /// Start through this bootstrapper instead (it picks the build and
    /// applies its own mods). Takes precedence over `executable`.
    pub via: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Launched {
    pub pid: Option<u32>,
}

#[derive(Debug, Clone)]
pub enum LaunchError {
    /// The stored session is missing or expired; the user must sign in again.
    SignInRequired(String),
    Other(String),
}

impl LaunchError {
    pub fn message(&self) -> &str {
        match self {
            LaunchError::SignInRequired(m) | LaunchError::Other(m) => m,
        }
    }
}

fn now_millis() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

pub async fn launch(request: LaunchRequest) -> Result<Launched, LaunchError> {
    let account = request.account;
    let token = tokio::task::spawn_blocking(move || credentials::load_session(account))
        .await
        .map_err(|e| LaunchError::Other(e.to_string()))?
        .map_err(LaunchError::SignInRequired)?;

    let ticket = roblox::authentication_ticket(&token).await.map_err(|e| {
        if e == "SESSION_EXPIRED" {
            LaunchError::SignInRequired("This account's session has expired. Sign in again.".into())
        } else {
            LaunchError::Other(e)
        }
    })?;

    let browser_tracker: u64 = rand::random::<u64>() % 900_000_000_000 + 100_000_000_000;

    let place_launcher = match (&request.server, &request.job) {
        (None, Some(job)) => format!(
            "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestGameJob&browserTrackerId={browser_tracker}&placeId={}&gameId={job}&isPlayTogetherGame=false&joinAttemptId={}&joinAttemptOrigin=publicServerListJoin",
            request.place_id,
            Uuid::new_v4()
        ),
        (None, None) => format!(
            "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestGame&browserTrackerId={browser_tracker}&placeId={}&isPlayTogetherGame=false&joinAttemptId={}&joinAttemptOrigin=PlayButton",
            request.place_id,
            Uuid::new_v4()
        ),
        (Some(link), _) => {
            let (place_id, code) = match link {
                ServerLink::LinkCode { place_id, code } => {
                    (place_id.unwrap_or(request.place_id), code.clone())
                }
                ServerLink::Share { code } => roblox::resolve_share_link(&token, code)
                    .await
                    .map_err(LaunchError::Other)?,
            };
            format!(
                "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestPrivateGame&browserTrackerId={browser_tracker}&placeId={place_id}&linkCode={code}&joinAttemptId={}&joinAttemptOrigin=PlayButton",
                Uuid::new_v4()
            )
        }
    };

    start_client(
        request.executable,
        request.via,
        ClientArgs {
            ticket,
            place_launcher,
            browser_tracker: browser_tracker.to_string(),
            launch_time: now_millis(),
        },
    )
    .await
}

/// What the Roblox client needs to join a game.
struct ClientArgs {
    ticket: String,
    place_launcher: String,
    browser_tracker: String,
    launch_time: String,
}

async fn start_client(executable: Option<PathBuf>, via: Option<PathBuf>, args: ClientArgs) -> Result<Launched, LaunchError> {
    let ClientArgs {
        ticket,
        place_launcher,
        browser_tracker,
        launch_time,
    } = args;

    let executable = if via.is_some() { None } else { executable };
    match executable {
        Some(exe) => {
            let child = std::process::Command::new(&exe)
                .current_dir(exe.parent().unwrap_or(std::path::Path::new(".")))
                .args([
                    "--app".to_owned(),
                    "-t".to_owned(),
                    ticket,
                    "-j".to_owned(),
                    place_launcher,
                    "-b".to_owned(),
                    browser_tracker,
                    format!("--launchtime={launch_time}"),
                    "--rloc".to_owned(),
                    "en_us".to_owned(),
                    "--gloc".to_owned(),
                    "en_us".to_owned(),
                ])
                .spawn()
                .map_err(|e| LaunchError::Other(format!("Couldn't start Roblox: {e}")))?;
            Ok(Launched { pid: Some(child.id()) })
        }
        None => {
            let before = process::roblox_pids();
            let uri = format!(
                "roblox-player:1+launchmode:play+gameinfo:{ticket}+launchtime:{launch_time}+placelauncherurl:{}+browsertrackerid:{browser_tracker}+robloxLocale:en_us+gameLocale:en_us",
                url::form_urlencoded::byte_serialize(place_launcher.as_bytes()).collect::<String>()
            );
            match &via {
                // Bootstrappers take the link the way Roblox's handler does.
                Some(bootstrapper) => {
                    std::process::Command::new(bootstrapper)
                        .args(["-player", &uri])
                        .spawn()
                        .map_err(|e| LaunchError::Other(format!("Couldn't start {}: {e}", bootstrapper.display())))?;
                }
                None => open::that_detached(&uri).map_err(|_| {
                    LaunchError::Other(
                        "Roblox doesn't appear to be installed. Install a version from Versions first."
                            .into(),
                    )
                })?,
            }

            // The handler starts the client asynchronously (a bootstrapper may
            // update Roblox first); watch for it.
            for _ in 0..if via.is_some() { 180 } else { 40 } {
                tokio::time::sleep(Duration::from_millis(500)).await;
                if let Some(pid) = process::roblox_pids()
                    .into_iter()
                    .find(|p| !before.contains(p) && claim(*p))
                {
                    return Ok(Launched { pid: Some(pid) });
                }
            }
            Ok(Launched { pid: None })
        }
    }
}

/// A `roblox-player:` link, as roblox.com opens it when you press Play.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebLaunch {
    pub ticket: String,
    pub place_launcher: String,
    pub browser_tracker: String,
    pub launch_time: String,
}

impl WebLaunch {
    /// Parses `roblox-player:1+launchmode:play+gameinfo:…+placelauncherurl:…`.
    pub fn parse(uri: &str) -> Option<Self> {
        let body = uri.trim().trim_matches('"').strip_prefix("roblox-player:")?;
        let mut ticket = None;
        let mut place_launcher = None;
        let mut browser_tracker = String::new();
        let mut launch_time = String::new();
        for part in body.split('+') {
            let Some((key, value)) = part.split_once(':') else { continue };
            match key.to_ascii_lowercase().as_str() {
                "gameinfo" => ticket = Some(value.to_owned()),
                "placelauncherurl" => {
                    place_launcher = url::form_urlencoded::parse(format!("x={value}").as_bytes())
                        .next()
                        .map(|(_, v)| v.into_owned())
                }
                "browsertrackerid" => browser_tracker = value.to_owned(),
                "launchtime" => launch_time = value.to_owned(),
                _ => {}
            }
        }
        Some(Self {
            ticket: ticket.filter(|t| !t.is_empty())?,
            place_launcher: place_launcher.filter(|p| !p.is_empty())?,
            browser_tracker,
            launch_time,
        })
    }

    fn query(&self, key: &str) -> Option<String> {
        let url = url::Url::parse(&self.place_launcher).ok()?;
        url.query_pairs()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.into_owned())
    }

    /// The place being joined.
    pub fn place_id(&self) -> Option<u64> {
        self.query("placeId").and_then(|v| v.parse().ok())
    }

    /// The server being joined, when the link names one.
    pub fn job_id(&self) -> Option<String> {
        self.query("gameId").filter(|v| !v.is_empty())
    }
}

/// Starts a client for a link roblox.com opened, using the session the
/// browser is signed in with. Always runs a specific build: handing the
/// link back to the system handler would just reopen Pious.
pub async fn launch_web(launch: WebLaunch, executable: PathBuf) -> Result<Launched, LaunchError> {
    let launch_time = if launch.launch_time.is_empty() { now_millis() } else { launch.launch_time };
    start_client(
        Some(executable),
        None,
        ClientArgs {
            ticket: launch.ticket,
            place_launcher: launch.place_launcher,
            browser_tracker: launch.browser_tracker,
            launch_time,
        },
    )
    .await
}

/// Claims a newly seen Roblox process for one launch, so two launches made
/// close together can't both adopt the same client.
fn claim(pid: u32) -> bool {
    static CLAIMED: std::sync::Mutex<Vec<u32>> = std::sync::Mutex::new(Vec::new());
    let mut claimed = CLAIMED.lock().unwrap_or_else(|e| e.into_inner());
    if claimed.contains(&pid) {
        return false;
    }
    claimed.push(pid);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_web_launch_links() {
        let uri = "roblox-player:1+launchmode:play+gameinfo:ABC+launchtime:123+placelauncherurl:https%3A%2F%2Fassetgame.roblox.com%2Fgame%2FPlaceLauncher.ashx%3Frequest%3DRequestGameJob%26placeId%3D606849621%26gameId%3Djob-1+browsertrackerid:42+robloxLocale:en_us";
        let launch = WebLaunch::parse(uri).unwrap();
        assert_eq!(launch.ticket, "ABC");
        assert_eq!(launch.place_id(), Some(606849621));
        assert_eq!(launch.job_id().as_deref(), Some("job-1"));
        assert_eq!(launch.browser_tracker, "42");
        assert!(WebLaunch::parse("roblox://placeId=1").is_none());
    }
}
