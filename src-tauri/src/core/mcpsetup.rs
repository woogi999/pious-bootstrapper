//! Connecting AI apps to Pious's MCP server in one click: Pious adds itself
//! to the app's own MCP settings file (keeping everything else in it, with
//! a backup of the old file next to it).
//!
//! Desktop apps start `pious.exe --mcp`, which relays to the running Pious.
//! ChatGPT only reaches servers on the internet, so it gets a link instead
//! (see `docs/MCP.md`).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The apps Pious can add itself to: ID, name.
pub const APPS: [(&str, &str); 4] = [
    ("claude", "Claude Desktop"),
    ("claude_code", "Claude Code"),
    ("cursor", "Cursor"),
    ("codex", "Codex"),
];

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// The settings file an app keeps its MCP servers in.
pub fn config_path(app: &str) -> Option<PathBuf> {
    Some(match app {
        "claude" => dirs::config_dir()?.join("Claude").join("claude_desktop_config.json"),
        "claude_code" => home().join(".claude.json"),
        "cursor" => home().join(".cursor").join("mcp.json"),
        "codex" => home().join(".codex").join("config.toml"),
        _ => return None,
    })
}

/// Whether the app's settings already have Pious.
pub fn is_connected(app: &str) -> bool {
    let Some(path) = config_path(app) else { return false };
    let Ok(text) = std::fs::read_to_string(&path) else { return false };
    if app == "codex" {
        return text.lines().any(|l| l.trim() == "[mcp_servers.pious]");
    }
    serde_json::from_str::<Value>(&text).is_ok_and(|v| v["mcpServers"]["pious"].is_object())
}

fn backup(path: &Path) {
    if path.is_file() {
        let _ = std::fs::copy(path, path.with_extension(format!("{}.pious-backup", path.extension().and_then(|e| e.to_str()).unwrap_or("bak"))));
    }
}

/// Adds Pious to the app's MCP settings. Returns the file changed.
pub fn connect(app: &str, exe: &Path) -> Result<PathBuf, String> {
    let path = config_path(app).ok_or("Pious doesn't know that app.")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Couldn't make {} ({e}).", parent.display()))?;
    }
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let exe = exe.display().to_string();
    let text = if app == "codex" {
        toml_with_pious(&existing, &exe)
    } else {
        let mut config: Value = if existing.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(&existing).map_err(|e| format!("{} isn't valid JSON, so Pious left it alone ({e}).", path.display()))?
        };
        if !config.is_object() {
            return Err(format!("{} isn't a settings object, so Pious left it alone.", path.display()));
        }
        if !config["mcpServers"].is_object() {
            config["mcpServers"] = json!({});
        }
        let mut server = json!({ "command": exe, "args": ["--mcp"] });
        if app == "claude_code" {
            server["type"] = json!("stdio");
        }
        config["mcpServers"]["pious"] = server;
        serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?
    };
    backup(&path);
    std::fs::write(&path, text).map_err(|e| format!("Couldn't write {} ({e}).", path.display()))?;
    Ok(path)
}

/// Takes Pious out of the app's MCP settings.
pub fn disconnect(app: &str) -> Result<(), String> {
    let path = config_path(app).ok_or("Pious doesn't know that app.")?;
    let Ok(existing) = std::fs::read_to_string(&path) else { return Ok(()) };
    let text = if app == "codex" {
        toml_without_pious(&existing)
    } else {
        let mut config: Value = serde_json::from_str(&existing).map_err(|e| e.to_string())?;
        if let Some(servers) = config["mcpServers"].as_object_mut() {
            servers.remove("pious");
        }
        serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?
    };
    backup(&path);
    std::fs::write(&path, text).map_err(|e| e.to_string())
}

/// Codex's config.toml without a `[mcp_servers.pious]` table (and its
/// sub-tables).
fn toml_without_pious(text: &str) -> String {
    let mut out = Vec::new();
    let mut skipping = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            skipping = trimmed == "[mcp_servers.pious]" || trimmed.starts_with("[mcp_servers.pious.");
        }
        if !skipping {
            out.push(line);
        }
    }
    let mut joined = out.join("\n");
    while joined.ends_with("\n\n") {
        joined.pop();
    }
    joined
}

fn toml_with_pious(text: &str, exe: &str) -> String {
    let mut out = toml_without_pious(text);
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() {
        out.push('\n');
    }
    // TOML literal strings keep Windows backslashes as they are.
    out.push_str(&format!("[mcp_servers.pious]\ncommand = '{exe}'\nargs = [\"--mcp\"]\n"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_table_is_replaced_not_duplicated() {
        let start = "model = \"o4\"\n\n[mcp_servers.other]\ncommand = \"x\"\n";
        let once = toml_with_pious(start, r"C:\Pious\pious.exe");
        let twice = toml_with_pious(&once, r"C:\Pious\pious.exe");
        assert_eq!(once, twice);
        assert!(twice.contains("[mcp_servers.other]"));
        assert!(twice.contains(r"command = 'C:\Pious\pious.exe'"));
        assert!(!toml_without_pious(&twice).contains("pious.exe"));
    }
}
