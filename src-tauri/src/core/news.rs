//! Roblox news for the News page: new Roblox client builds (from
//! weao.xyz), Roblox's own announcements and release notes, and anything
//! about Hyperion (Byfron), Roblox's anti-cheat, from the Developer Forum.

use serde::{Deserialize, Serialize};

const FORUM: &str = "https://devforum.roblox.com";

/// One news item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewsItem {
    /// Unique across sources ("forum-123", "build-version-abc").
    pub id: String,
    /// "updates" (Roblox client builds), "roblox" (announcements and
    /// release notes) or "hyperion" (the anti-cheat).
    pub kind: String,
    /// Where it's from, in words ("Announcements", "Release notes"…).
    pub source: String,
    pub title: String,
    pub url: String,
    /// RFC 3339, when known.
    pub date: Option<String>,
    pub summary: Option<String>,
}

#[derive(Deserialize)]
struct TopicList {
    topic_list: Topics,
}

#[derive(Deserialize)]
struct Search {
    #[serde(default)]
    topics: Vec<Topic>,
}

#[derive(Deserialize)]
struct Topics {
    topics: Vec<Topic>,
}

#[derive(Deserialize)]
struct Topic {
    id: u64,
    title: String,
    slug: String,
    created_at: Option<String>,
    #[serde(default)]
    excerpt: Option<String>,
    #[serde(default)]
    pinned: bool,
}

fn forum_item(topic: Topic, kind: &str, source: &str) -> NewsItem {
    NewsItem {
        id: format!("forum-{}", topic.id),
        kind: kind.into(),
        source: source.into(),
        url: format!("{FORUM}/t/{}/{}", topic.slug, topic.id),
        title: topic.title,
        date: topic.created_at,
        summary: topic.excerpt.map(|e| clean(&e)),
    }
}

/// Forum excerpts are HTML-escaped text.
fn clean(text: &str) -> String {
    text.replace("&hellip;", "…")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

async fn get<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    crate::core::roblox::client()
        .get(url)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

/// The newest topics of a forum category, without its pinned "About" post.
async fn category(path: &str, kind: &str, source: &str) -> Result<Vec<NewsItem>, String> {
    let list: TopicList = get(&format!("{FORUM}/c/{path}/l/latest.json")).await?;
    Ok(list
        .topic_list
        .topics
        .into_iter()
        .filter(|t| !(t.pinned && t.title.starts_with("About the")))
        .take(15)
        .map(|t| forum_item(t, kind, source))
        .collect())
}

/// Forum topics with a word in their title, newest first.
async fn titled(word: &str) -> Result<Vec<NewsItem>, String> {
    let search: Search = get(&format!("{FORUM}/search.json?q={word}%20in%3Atitle%20order%3Alatest")).await?;
    Ok(search.topics.into_iter().take(12).map(|t| forum_item(t, "hyperion", "Developer Forum")).collect())
}

/// Roblox client builds: what's live now, before, and coming next.
async fn builds() -> Result<Vec<NewsItem>, String> {
    let builds = crate::core::roblox::weao_builds().await?;
    Ok(builds
        .into_iter()
        .filter(|b| b.kind != "History")
        .map(|b| {
            let (title, summary) = match b.kind.as_str() {
                "Upcoming" => (
                    format!("Roblox {} is coming", b.version),
                    "The next Windows build is out for testing. Bootstrappers and mods may need updates once it's live.",
                ),
                "Previous" => (format!("Roblox {}", b.version), "The Windows build before the current one."),
                _ => (format!("Roblox updated to {}", b.version), "The Windows build everyone gets now."),
            };
            NewsItem {
                id: format!("build-{}", b.hash),
                kind: "updates".into(),
                source: "Roblox client".into(),
                title,
                url: "https://weao.xyz".into(),
                date: parse_weao_date(&b.date),
                summary: Some(format!("{summary} ({})", b.hash)),
            }
        })
        .collect())
}

/// weao.xyz writes dates like "10/7/2026, 5:02:30 PM UTC".
fn parse_weao_date(text: &str) -> Option<String> {
    let trimmed = text.trim().trim_end_matches("UTC").trim();
    chrono::NaiveDateTime::parse_from_str(trimmed, "%m/%d/%Y, %I:%M:%S %p")
        .ok()
        .map(|d| d.and_utc().to_rfc3339())
        .or_else(|| chrono::DateTime::parse_from_rfc3339(text).ok().map(|d| d.to_rfc3339()))
}

/// Everything, newest first. Sources that fail are skipped (and named in
/// the errors) so one being down doesn't hide the rest.
pub async fn fetch() -> (Vec<NewsItem>, Vec<String>) {
    let (updates, announcements, notes, hyperion, byfron) = futures::join!(
        builds(),
        category("updates/announcements/36", "roblox", "Announcements"),
        category("updates/release-notes/62", "roblox", "Release notes"),
        titled("hyperion"),
        titled("byfron"),
    );
    let mut items = Vec::new();
    let mut errors = Vec::new();
    for (name, result) in [
        ("Roblox builds", updates),
        ("Announcements", announcements),
        ("Release notes", notes),
        ("Hyperion", hyperion),
        ("Byfron", byfron),
    ] {
        match result {
            Ok(found) => items.extend(found),
            Err(error) => errors.push(format!("{name}: {error}")),
        }
    }
    items.sort_by(|a, b| b.date.cmp(&a.date));
    items.dedup_by(|a, b| a.id == b.id);
    (items, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_weao_dates() {
        assert_eq!(parse_weao_date("10/7/2026, 5:02:30 PM UTC").as_deref(), Some("2026-10-07T17:02:30+00:00"));
        assert_eq!(parse_weao_date("not a date"), None);
    }
}
