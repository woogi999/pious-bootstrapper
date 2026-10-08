//! The News page (and the news on Home): Roblox builds, announcements and
//! Hyperion (Byfron) posts, kept from last time until fresh ones arrive.

use std::time::{Duration, Instant};

use serde::Serialize;

use super::{Service, Shared};
use crate::core::news::{self, NewsItem};

/// How long fetched news counts as fresh.
const FRESH: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone, Default, Serialize)]
pub struct News {
    pub items: Vec<NewsItem>,
    pub loading: bool,
    /// Sources that couldn't be reached last time.
    pub errors: Vec<String>,
    #[serde(skip)]
    pub fetched: Option<Instant>,
}

impl Service {
    /// Fetches the news, unless it's fresh (`force` fetches anyway).
    pub async fn refresh_news(self: &Shared, force: bool) {
        let skip = self.mutate(|s| {
            let fresh = s.news.fetched.is_some_and(|t| t.elapsed() < FRESH);
            let skip = s.news.loading || (fresh && !force);
            if !skip {
                s.news.loading = true;
            }
            skip
        });
        if skip {
            return;
        }
        let (items, errors) = news::fetch().await;
        if !items.is_empty() {
            crate::core::cache::save("news", &items);
        }
        self.mutate(|s| {
            s.news.loading = false;
            s.news.errors = errors;
            s.news.fetched = Some(Instant::now());
            if !items.is_empty() {
                s.news.items = items;
            }
        });
    }
}
