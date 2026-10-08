//! Internet settings for Roblox: which DNS servers look up Roblox's
//! addresses, and checking the connection to Roblox.
//!
//! Windows can't give one program its own DNS servers, but its Name
//! Resolution Policy Table can send the lookups for some domains to chosen
//! servers. Pious adds rules only for Roblox's domains, so the rest of the
//! PC keeps its usual DNS. The rules carry the comment "Pious", and Pious
//! only ever touches those. Changing them needs administrator rights, so
//! Windows asks once each time.

use std::net::ToSocketAddrs;
use std::time::{Duration, Instant};

use serde::Serialize;

/// The domains Roblox uses: the site and its APIs, game servers' matchmaking,
/// and the content (assets, thumbnails, installers).
pub const DOMAINS: [&str; 4] = [".roblox.com", ".rbxcdn.com", ".rbx.com", ".robloxlabs.com"];
const COMMENT: &str = "Pious";

/// Known DNS providers: (id, name, servers).
pub const PROVIDERS: [(&str, &str, [&str; 2]); 6] = [
    ("cloudflare", "Cloudflare", ["1.1.1.1", "1.0.0.1"]),
    ("google", "Google", ["8.8.8.8", "8.8.4.4"]),
    ("quad9", "Quad9", ["9.9.9.9", "149.112.112.112"]),
    ("opendns", "OpenDNS", ["208.67.222.222", "208.67.220.220"]),
    ("adguard", "AdGuard", ["94.140.14.14", "94.140.15.15"]),
    ("controld", "Control D", ["76.76.2.0", "76.76.10.0"]),
];

/// The servers a choice means: a provider's, or the custom list. Empty for
/// "your network's DNS".
pub fn servers(choice: &str, custom: &[String]) -> Result<Vec<String>, String> {
    if choice == "auto" || choice.is_empty() {
        return Ok(Vec::new());
    }
    if choice == "custom" {
        let list: Vec<String> = custom.iter().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect();
        if list.is_empty() {
            return Err("Type at least one DNS server address.".into());
        }
        for server in &list {
            if server.parse::<std::net::IpAddr>().is_err() {
                return Err(format!("{server} isn't an IP address (like 1.1.1.1)."));
            }
        }
        return Ok(list);
    }
    PROVIDERS
        .iter()
        .find(|(id, _, _)| *id == choice)
        .map(|(_, _, s)| s.iter().map(|s| (*s).to_owned()).collect())
        .ok_or_else(|| "Unknown DNS choice.".into())
}

/// The servers Roblox's lookups go to now (Pious's rules), if any.
pub fn current() -> Vec<String> {
    let script = format!(
        "Get-DnsClientNrptRule | Where-Object {{ $_.Comment -eq '{COMMENT}' }} | Select-Object -First 1 -ExpandProperty NameServers"
    );
    run_hidden(&script)
        .map(|out| out.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned).collect())
        .unwrap_or_default()
}

/// Points Roblox's lookups at `servers` (none: back to the network's DNS).
/// Asks Windows for administrator rights.
pub fn set(servers: &[String]) -> Result<(), String> {
    for server in servers {
        if server.parse::<std::net::IpAddr>().is_err() {
            return Err(format!("{server} isn't an IP address."));
        }
    }
    let mut script = format!("$ErrorActionPreference = 'Stop'; Get-DnsClientNrptRule | Where-Object {{ $_.Comment -eq '{COMMENT}' }} | Remove-DnsClientNrptRule -Force; ");
    if !servers.is_empty() {
        let namespaces = DOMAINS.iter().map(|d| format!("'{d}'")).collect::<Vec<_>>().join(",");
        let list = servers.iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(",");
        script.push_str(&format!("Add-DnsClientNrptRule -Namespace {namespaces} -NameServers {list} -Comment '{COMMENT}'; "));
    }
    script.push_str("Clear-DnsClientCache");
    run_elevated(&script)
}

/// Forgets every address Windows has looked up (no administrator rights
/// needed), so the next lookups are fresh.
pub fn flush() -> Result<(), String> {
    let mut command = std::process::Command::new("ipconfig");
    command.arg("/flushdns");
    hide(&mut command);
    let status = command.status().map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err("Windows couldn't clear its DNS cache.".into()) }
}

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    /// How long looking up roblox.com took, in milliseconds.
    pub lookup_ms: Option<u64>,
    /// The addresses it found.
    pub addresses: Vec<String>,
    /// How long Roblox's servers took to answer, in milliseconds.
    pub response_ms: Option<u64>,
    pub error: Option<String>,
    /// The DNS servers Roblox's lookups use, when Pious set them.
    pub servers: Vec<String>,
}

/// Looks up roblox.com and asks Roblox's API for a reply, timing both.
pub async fn check() -> Check {
    let servers = tokio::task::spawn_blocking(current).await.unwrap_or_default();
    let lookup = tokio::task::spawn_blocking(|| {
        let started = Instant::now();
        let found = ("apis.roblox.com", 443).to_socket_addrs();
        (started.elapsed(), found.map(|a| a.map(|a| a.ip().to_string()).collect::<Vec<_>>()))
    })
    .await;
    let (lookup_ms, addresses, mut error) = match lookup {
        Ok((took, Ok(mut found))) => {
            found.dedup();
            (Some(took.as_millis() as u64), found, None)
        }
        Ok((_, Err(e))) => (None, Vec::new(), Some(format!("Couldn't look up Roblox's address ({e})."))),
        Err(e) => (None, Vec::new(), Some(e.to_string())),
    };
    let mut response_ms = None;
    if error.is_none() {
        let client = crate::core::roblox::client();
        let started = Instant::now();
        match client.get("https://apis.roblox.com/universes/v1/places/1818/universe").timeout(Duration::from_secs(8)).send().await {
            Ok(_) => response_ms = Some(started.elapsed().as_millis() as u64),
            Err(e) => error = Some(format!("Roblox didn't answer ({e}).")),
        }
    }
    Check { lookup_ms, addresses, response_ms, error, servers }
}

fn hide(command: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    let _ = command;
}

fn run_hidden(script: &str) -> Result<String, String> {
    let mut command = std::process::Command::new("powershell.exe");
    command.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script]);
    hide(&mut command);
    let out = command.output().map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Runs a PowerShell script as administrator (Windows asks first) and
/// waits for it.
fn run_elevated(script: &str) -> Result<(), String> {
    // Passed encoded, so no quoting can break it on the way.
    let encoded = {
        use base64::Engine;
        let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
        base64::engine::general_purpose::STANDARD.encode(utf16)
    };
    let outer = format!(
        "try {{ $p = Start-Process powershell.exe -Verb RunAs -WindowStyle Hidden -Wait -PassThru -ArgumentList '-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-EncodedCommand','{encoded}'; exit $p.ExitCode }} catch {{ exit 1223 }}"
    );
    let mut command = std::process::Command::new("powershell.exe");
    command.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &outer]);
    hide(&mut command);
    let status = command.status().map_err(|e| e.to_string())?;
    match status.code() {
        Some(0) => Ok(()),
        Some(1223) => Err("Windows didn't allow it (the administrator prompt was declined).".into()),
        _ => Err("Windows couldn't change the DNS settings.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_become_servers() {
        assert!(servers("auto", &[]).unwrap().is_empty());
        assert_eq!(servers("cloudflare", &[]).unwrap(), ["1.1.1.1", "1.0.0.1"]);
        assert_eq!(servers("custom", &[" 9.9.9.9 ".into(), "".into()]).unwrap(), ["9.9.9.9"]);
        assert!(servers("custom", &["dns.example".into()]).is_err());
        assert!(servers("custom", &[]).is_err());
    }
}
