//! Crash reports, kept as plain text files in `<data>\crashes`: Pious's own
//! (a panic hook) and Roblox's (a window whose process ended with a crash
//! code). Nothing is sent anywhere; Settings → About lists them, and the
//! user can open one or report it on GitHub themselves.
//!
//! Reports leave out what's private: the Windows user name is replaced by
//! `%USERPROFILE%`, and anything that looks like a Roblox session token is
//! cut out.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::core::store;

/// Most reports kept; older ones are deleted.
const KEEP: usize = 30;

pub fn dir() -> PathBuf {
    store::data_dir().join("crashes")
}

/// One crash report, as Settings lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    pub file: PathBuf,
    /// "pious" or "roblox".
    pub kind: String,
    /// When it happened (from the file name), RFC 3339.
    pub at: String,
    /// The first line that says what happened.
    pub summary: String,
}

/// Takes private things out of a report.
pub fn redact(text: &str) -> String {
    let mut out = text.to_owned();
    if let Some(home) = dirs::home_dir() {
        let home = home.display().to_string();
        if home.len() > 3 {
            out = out.replace(&home, "%USERPROFILE%");
            out = out.replace(&home.replace('\\', "/"), "%USERPROFILE%");
        }
    }
    if let Ok(user) = std::env::var("USERNAME") {
        if user.len() > 2 {
            out = out.replace(&format!("\\{user}\\"), "\\%USERNAME%\\");
        }
    }
    // Roblox session tokens start with this warning.
    const COOKIE: &str = "_|WARNING:-DO-NOT-SHARE-THIS.";
    while let Some(start) = out.find(COOKIE) {
        let end = out[start..].find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ';').map_or(out.len(), |e| start + e);
        out.replace_range(start..end, "[session token removed]");
    }
    out
}

fn write(kind: &str, body: &str) -> Option<PathBuf> {
    let dir = dir();
    std::fs::create_dir_all(&dir).ok()?;
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let path = dir.join(format!("{kind}-{stamp}-{}.txt", &uuid::Uuid::new_v4().simple().to_string()[..6]));
    let header = format!(
        "Pious {} crash report ({kind})\r\nWhen: {}\r\nWindows: {}\r\n\r\n",
        crate::core::updater::CURRENT,
        chrono::Utc::now().to_rfc3339(),
        windows_version(),
    );
    std::fs::write(&path, redact(&format!("{header}{body}"))).ok()?;
    prune(&dir);
    Some(path)
}

/// Keeps the newest [`KEEP`] reports.
fn prune(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "txt")).collect();
    // Names start with the kind, then the time: sort by the time part.
    files.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().split_once('-').map(|(_, rest)| rest.to_owned()).unwrap_or_default()));
    while files.len() > KEEP {
        let _ = std::fs::remove_file(files.remove(0));
    }
}

/// "Windows 11 Pro 24H2 (build 26100)", from the registry.
pub fn windows_version() -> String {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
        let read = |name: &str| -> Option<String> {
            let key: Vec<u16> = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion".encode_utf16().chain([0]).collect();
            let value: Vec<u16> = name.encode_utf16().chain([0]).collect();
            let mut buffer = vec![0u16; 256];
            let mut size = (buffer.len() * 2) as u32;
            let ok = unsafe {
                RegGetValueW(HKEY_LOCAL_MACHINE, key.as_ptr(), value.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buffer.as_mut_ptr().cast(), &mut size)
            };
            (ok == 0).then(|| String::from_utf16_lossy(&buffer[..buffer.iter().position(|&c| c == 0).unwrap_or(0)]))
        };
        let build = read("CurrentBuild").unwrap_or_default();
        // Windows 11 still calls itself Windows 10 in ProductName.
        let product = read("ProductName").unwrap_or_else(|| "Windows".into());
        let product = if build.parse::<u32>().is_ok_and(|b| b >= 22000) { product.replace("Windows 10", "Windows 11") } else { product };
        format!("{product} {} (build {build})", read("DisplayVersion").unwrap_or_default())
    }
    #[cfg(not(windows))]
    {
        std::env::consts::OS.to_owned()
    }
}

/// Writes a report whenever Pious itself panics, then lets the panic go on
/// as before. Panics in background tasks would otherwise vanish silently.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current().name().unwrap_or("unnamed").to_owned();
        let backtrace = std::backtrace::Backtrace::force_capture();
        let body = format!("Thread: {thread}\r\nWhat: {info}\r\n\r\nBacktrace:\r\n{backtrace}\r\n{}", recent_ui_errors());
        let _ = write("pious", &body);
        previous(info);
    }));
}

/// The end of the interface's error log, which often says what led up to it.
fn recent_ui_errors() -> String {
    let Ok(text) = std::fs::read_to_string(store::data_dir().join("ui-errors.log")) else { return String::new() };
    let lines: Vec<&str> = text.lines().rev().take(20).collect();
    if lines.is_empty() {
        return String::new();
    }
    format!("\r\nRecent interface errors (newest first):\r\n{}\r\n", lines.join("\r\n"))
}

/// Whether a process exit code means it crashed (a Windows exception like
/// an access violation), rather than closing normally or being closed.
pub fn is_crash_code(code: u32) -> bool {
    // NTSTATUS errors (0xC0000000 and up), e.g. 0xC0000005 access violation,
    // 0xC0000409 stack buffer overrun; also "abort" (3).
    code >= 0xC000_0000 || code == 3
}

pub fn describe_code(code: u32) -> String {
    let what = match code {
        0xC000_0005 => "an access violation",
        0xC000_0409 => "a stack buffer overrun",
        0xC000_00FD => "a stack overflow",
        0xC000_0374 => "heap corruption",
        0xC000_001D => "an illegal instruction",
        0xC000_0094 => "an integer division by zero",
        0xC000_0017 | 0xC000_009A => "running out of memory",
        3 => "an abort",
        _ => "an unexpected error",
    };
    format!("{what} (exit code 0x{code:08X})")
}

/// Writes a report for a Roblox window that crashed: what was being played
/// and the end of its log.
pub fn report_roblox(game: &str, place_id: u64, version: Option<&str>, code: u32, log: Option<&Path>, minutes: i64) -> Option<PathBuf> {
    let mut body = format!(
        "Roblox crashed with {}.\r\nGame: {game} (place {place_id})\r\nRoblox version: {}\r\nPlayed for: {minutes} min\r\n",
        describe_code(code),
        version.unwrap_or("unknown"),
    );
    if let Some(log) = log {
        if let Ok(bytes) = std::fs::read(log) {
            let text = String::from_utf8_lossy(&bytes);
            let tail: Vec<&str> = text.lines().rev().take(60).collect::<Vec<_>>().into_iter().rev().collect();
            body.push_str(&format!("\r\nLast lines of Roblox's log ({}):\r\n{}\r\n", log.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(), tail.join("\r\n")));
        }
    }
    write("roblox", &body)
}

/// Every report, newest first.
pub fn list() -> Vec<Report> {
    let Ok(entries) = std::fs::read_dir(dir()) else { return Vec::new() };
    let mut out: Vec<Report> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "txt"))
        .filter_map(|file| {
            let name = file.file_stem()?.to_string_lossy().into_owned();
            let (kind, rest) = name.split_once('-')?;
            let at = chrono::NaiveDateTime::parse_from_str(rest.get(..15)?, "%Y%m%d-%H%M%S").ok()?.and_utc().to_rfc3339();
            let text = std::fs::read_to_string(&file).unwrap_or_default();
            let summary = text
                .lines()
                .find(|l| l.starts_with("What:") || l.starts_with("Roblox crashed"))
                .unwrap_or_default()
                .trim_start_matches("What:")
                .trim()
                .chars()
                .take(200)
                .collect();
            Some(Report { file, kind: kind.to_owned(), at, summary })
        })
        .collect();
    out.sort_by(|a, b| b.at.cmp(&a.at));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_tokens_and_home() {
        let text = "cookie=_|WARNING:-DO-NOT-SHARE-THIS.--Sharing-this-will-allow|_ABCDEF123; next";
        let out = redact(text);
        assert!(!out.contains("ABCDEF123"), "{out}");
        assert!(out.contains("[session token removed]"));
        assert!(out.ends_with("; next"));
        if let Some(home) = dirs::home_dir() {
            let out = redact(&format!("at {}\\x.rs", home.display()));
            assert!(out.contains("%USERPROFILE%"), "{out}");
        }
    }

    #[test]
    fn tells_crashes_from_normal_exits() {
        assert!(is_crash_code(0xC000_0005));
        assert!(!is_crash_code(0));
        assert!(!is_crash_code(1));
        assert!(describe_code(0xC000_0005).contains("access violation"));
    }
}
