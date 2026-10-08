//! Game recommendations built from the play history of every account.
//!
//! Each game an account has played becomes a weighted "seed" (recent and
//! frequent play counts more, and a game played on several accounts counts
//! for each). Roblox is asked for experiences similar to the strongest
//! seeds, and candidates are scored by how many seeds point at them, how
//! highly they rank for each, and how well liked they are.

use std::collections::{HashMap, HashSet};

use crate::core::roblox::{self, SuggestedGame};

/// A game the accounts have played, and how much it should count.
#[derive(Debug, Clone)]
pub struct Seed {
    pub universe_id: u64,
    pub name: String,
    pub weight: f32,
    /// Accounts (labels) that played it.
    pub accounts: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Recommendation {
    pub universe_id: u64,
    pub place_id: u64,
    pub name: String,
    pub creator: String,
    pub players: u64,
    /// Share of positive votes, 0–1.
    pub rating: Option<f32>,
    pub score: f32,
    /// Seed games that led here, strongest first.
    pub because: Vec<String>,
    /// Accounts whose history led here.
    pub accounts: Vec<String>,
}

impl Recommendation {
    pub fn artwork_key(&self) -> String {
        format!("game-{}", self.place_id)
    }
}

const SEEDS: usize = 14;

pub async fn recommend(mut seeds: Vec<Seed>, exclude: HashSet<u64>, limit: usize) -> Result<Vec<Recommendation>, String> {
    if seeds.is_empty() {
        return Ok(Vec::new());
    }
    seeds.sort_by(|a, b| b.weight.total_cmp(&a.weight));
    seeds.truncate(SEEDS);

    let results = futures::future::join_all(seeds.iter().map(|s| roblox::similar_games(s.universe_id))).await;
    if results.iter().all(Result::is_err) {
        return Err(results.into_iter().find_map(Result::err).unwrap_or_default());
    }

    struct Candidate {
        game: SuggestedGame,
        score: f32,
        because: Vec<(String, f32)>,
        accounts: HashSet<String>,
    }

    let seed_ids: HashSet<u64> = seeds.iter().map(|s| s.universe_id).collect();
    let mut candidates: HashMap<u64, Candidate> = HashMap::new();

    for (seed, result) in seeds.iter().zip(results) {
        let Ok(games) = result else { continue };
        for (rank, game) in games.into_iter().enumerate() {
            if exclude.contains(&game.universe_id) || seed_ids.contains(&game.universe_id) {
                continue;
            }
            let votes = game.up_votes + game.down_votes;
            let liked = if votes > 0 { game.up_votes as f32 / votes as f32 } else { 0.75 };
            let contribution = seed.weight
                * (1.0 / (1.0 + 0.22 * rank as f32))
                * (0.55 + 0.45 * liked)
                * (1.0 + 0.04 * (1.0 + game.players as f32).ln());

            let entry = candidates.entry(game.universe_id).or_insert_with(|| Candidate {
                game: game.clone(),
                score: 0.0,
                because: Vec::new(),
                accounts: HashSet::new(),
            });
            entry.score += contribution;
            entry.because.push((seed.name.clone(), contribution));
            entry.accounts.extend(seed.accounts.iter().cloned());
        }
    }

    let mut out: Vec<Recommendation> = candidates
        .into_values()
        .map(|mut c| {
            c.because.sort_by(|a, b| b.1.total_cmp(&a.1));
            let mut seen = HashSet::new();
            c.because.retain(|(name, _)| !name.is_empty() && seen.insert(name.clone()));
            let votes = c.game.up_votes + c.game.down_votes;
            let mut accounts: Vec<String> = c.accounts.into_iter().collect();
            accounts.sort();
            Recommendation {
                universe_id: c.game.universe_id,
                place_id: c.game.place_id,
                name: c.game.name,
                creator: c.game.creator,
                players: c.game.players,
                rating: (votes > 50).then(|| c.game.up_votes as f32 / votes as f32),
                score: c.score,
                because: c.because.into_iter().map(|(n, _)| n).collect(),
                accounts,
            }
        })
        .collect();

    out.sort_by(|a, b| b.score.total_cmp(&a.score));
    out.truncate(limit);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn no_seeds_means_no_recommendations() {
        let recs = recommend(Vec::new(), HashSet::new(), 8).await.unwrap();
        assert!(recs.is_empty());
    }
}
