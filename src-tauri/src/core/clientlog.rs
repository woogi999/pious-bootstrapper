//! Reading the Roblox client's own log, the same way bootstrappers do, to
//! learn which server a window joined and when it was disconnected.

use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Disconnect reason Roblox logs when the player left on purpose.
pub const LEFT_ON_PURPOSE: u32 = 285;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogEvent {
    /// Joined a server (also after teleporting).
    Joined { job: String, place: u64 },
    /// Disconnected, with Roblox's reason code (285 = left on purpose).
    Disconnected { reason: u32 },
    /// The server's public address (logged as it connects).
    Server { ip: String },
    /// A FastFlag from ClientAppSettings.json that Roblox refused (it isn't
    /// on Roblox's allowlist).
    Denied { flag: String },
}

pub fn logs_dir() -> Option<PathBuf> {
    Some(dirs::data_local_dir()?.join("Roblox").join("logs"))
}

/// The newest player log created since `since` that isn't in `taken`.
/// Each Roblox window writes its own log right after it starts.
pub fn find_log(since: SystemTime, taken: &[PathBuf]) -> Option<PathBuf> {
    let dir = logs_dir()?;
    let earliest = since.checked_sub(Duration::from_secs(5)).unwrap_or(since);
    let mut candidates: Vec<(SystemTime, PathBuf)> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            name.contains("_Player_") && !name.contains("CrashHandler") && name.ends_with(".log")
        })
        .filter(|p| !taken.contains(p))
        .filter_map(|p| {
            let meta = std::fs::metadata(&p).ok()?;
            let created = meta.created().or_else(|_| meta.modified()).ok()?;
            (created >= earliest).then_some((created, p))
        })
        .collect();
    // The first log written after the launch belongs to it.
    candidates.sort();
    candidates.into_iter().next().map(|(_, p)| p)
}

/// Follows one log file, returning only lines written since the last read.
#[derive(Debug, Clone)]
pub struct LogTail {
    pub path: PathBuf,
    offset: u64,
    partial: String,
}

impl LogTail {
    pub fn new(path: PathBuf) -> Self {
        Self { path, offset: 0, partial: String::new() }
    }

    pub fn read(&mut self) -> Vec<LogEvent> {
        let Ok(mut file) = std::fs::File::open(&self.path) else {
            return Vec::new();
        };
        if file.seek(SeekFrom::Start(self.offset)).is_err() {
            return Vec::new();
        }
        let mut bytes = Vec::new();
        if file.read_to_end(&mut bytes).is_err() {
            return Vec::new();
        }
        self.offset += bytes.len() as u64;
        self.partial.push_str(&String::from_utf8_lossy(&bytes));

        // Keep an unfinished last line for next time.
        let complete = match self.partial.rfind('\n') {
            Some(end) => {
                let rest = self.partial.split_off(end + 1);
                std::mem::replace(&mut self.partial, rest)
            }
            None => return Vec::new(),
        };
        complete.lines().filter_map(parse).collect()
    }
}

fn parse(line: &str) -> Option<LogEvent> {
    if let Some(rest) = line.split("Denied local configuration for: ").nth(1) {
        let flag = rest.trim();
        return (!flag.is_empty()).then(|| LogEvent::Denied { flag: flag.to_owned() });
    }
    if let Some(rest) = line.split("! Joining game '").nth(1) {
        let (job, rest) = rest.split_once('\'')?;
        let place = rest.split(" place ").nth(1)?.split_whitespace().next()?.parse().ok()?;
        return Some(LogEvent::Joined { job: job.to_owned(), place });
    }
    if let Some(rest) = line.split("UDMUX Address = ").nth(1) {
        let ip = rest.split([',', ' ', '|']).next()?.trim();
        if ip.parse::<std::net::IpAddr>().is_ok() {
            return Some(LogEvent::Server { ip: ip.to_owned() });
        }
        return None;
    }
    if let Some(rest) = line.split("Disconnection Notification. Reason: ").nth(1) {
        let reason = rest.trim().split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()?;
        return Some(LogEvent::Disconnected { reason });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_denied_flags() {
        let line = "2026-10-08T19:01:08.499Z,0.499894,0c50,6,Warning [FLog::FlagFetchingStarterModule] Denied local configuration for: DFIntTaskSchedulerTargetFps";
        assert_eq!(parse(line), Some(LogEvent::Denied { flag: "DFIntTaskSchedulerTargetFps".into() }));
    }

    #[test]
    fn parses_log_lines() {
        assert_eq!(
            parse("2026-10-07T17:30:31.796Z,2.796126,32e8,6 [FLog::Output] ! Joining game 'bd6d873a-6f5a' place 9391468976 at 10.32.39.146"),
            Some(LogEvent::Joined { job: "bd6d873a-6f5a".into(), place: 9391468976 })
        );
        assert_eq!(
            parse("2026-10-07T17:32:26.372Z,117.372963,5cf8,7 [FLog::Network] Disconnection Notification. Reason: 277"),
            Some(LogEvent::Disconnected { reason: 277 })
        );
        assert_eq!(parse("[FLog::Network] Replicator created"), None);
        assert_eq!(
            parse("2026-10-07T17:30:32.1Z,3.1,32e8,6 [FLog::Network] UDMUX Address = 128.116.21.4, Port = 56722 | RCC Server Address = 10.32.39.146, Port = 21022"),
            Some(LogEvent::Server { ip: "128.116.21.4".into() })
        );
    }

    #[test]
    fn tails_only_new_complete_lines() {
        let path = std::env::temp_dir().join(format!("pious-log-{}.log", uuid::Uuid::new_v4().simple()));
        std::fs::write(&path, "x ! Joining game 'a' place 1 at y\nDisconnection Notifi").unwrap();
        let mut tail = LogTail::new(path.clone());
        assert_eq!(tail.read(), vec![LogEvent::Joined { job: "a".into(), place: 1 }]);
        let mut file = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        std::io::Write::write_all(&mut file, b"cation. Reason: 279\n").unwrap();
        assert_eq!(tail.read(), vec![LogEvent::Disconnected { reason: 279 }]);
        assert_eq!(tail.read(), vec![]);
        let _ = std::fs::remove_file(path);
    }
}
