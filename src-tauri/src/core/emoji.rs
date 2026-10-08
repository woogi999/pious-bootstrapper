//! Discord-style emoji shortcodes (`:sob:` → 😭). The table is shared with
//! the interface (`src/lib/emoji-shortcodes.json`, from iamcal's
//! emoji-datasource, the same names Discord uses).

use std::sync::OnceLock;

const TABLE: &str = include_str!("../../../src/lib/emoji-shortcodes.json");

/// (shortcode, emoji), in the table's order (most common first).
fn table() -> &'static [(String, String)] {
    static PARSED: OnceLock<Vec<(String, String)>> = OnceLock::new();
    PARSED.get_or_init(|| serde_json::from_str(TABLE).unwrap_or_default())
}

/// A character that can be part of a shortcode.
pub fn is_shortcode_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '+' || c == '-'
}

/// The emoji for an exact shortcode (without colons).
pub fn exact(code: &str) -> Option<&'static str> {
    let code = code.to_ascii_lowercase();
    table().iter().find(|(name, _)| *name == code).map(|(_, emoji)| emoji.as_str())
}

/// Up to `limit` shortcodes for what's been typed after the colon: ones
/// that start with it first, then ones that contain it.
pub fn search(query: &str, limit: usize) -> Vec<(&'static str, &'static str)> {
    let query = query.to_ascii_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let all = table();
    let starts = all.iter().filter(|(name, _)| name.starts_with(&query));
    let contains = all.iter().filter(|(name, _)| !name.starts_with(&query) && name.contains(&query));
    starts.chain(contains).take(limit).map(|(n, e)| (n.as_str(), e.as_str())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_discord_names() {
        assert_eq!(exact("sob"), Some("😭"));
        assert_eq!(exact("SKULL"), Some("💀"));
        assert_eq!(exact("+1"), Some("👍"));
        assert_eq!(search("so", 3)[0].0, "sob");
        assert!(search("", 5).is_empty());
    }
}
