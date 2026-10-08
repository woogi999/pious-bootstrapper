//! Games, private servers, collections, recommendations and artwork.

use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use uuid::Uuid;

use super::{Service, Shared, Tone};
use crate::core::model::{Game, LaunchConfig, PrivateServer, ServerChoice, VersionChoice, parse_place_id, parse_server_link};
use crate::core::recommend::{self, Seed};
use crate::core::roblox::{self, GameInfo};
use crate::core::{credentials, store};

/// The private server form, as the window sends it.
#[derive(Debug, Deserialize)]
pub struct ServerForm {
    pub editing: Option<Uuid>,
    pub game: Option<Uuid>,
    pub name: String,
    pub link: String,
    pub server_id: String,
    pub notes: String,
}

impl Service {
    // ── Games ────────────────────────────────────────────────────────────

    pub async fn look_up_game(&self, input: String) -> Result<GameInfo, String> {
        let place = parse_place_id(&input).ok_or("Enter a place ID or a roblox.com/games link.")?;
        roblox::fetch_game(place).await
    }

    /// Adds a looked-up game to the library (or marks an already played one
    /// as added). Returns its ID.
    pub fn add_game(self: &Shared, info: GameInfo) -> Uuid {
        let (id, message) = self.mutate(|s| {
            let now = s.now();
            if let Some(existing) = s.bootstrapper.games.iter_mut().find(|g| g.place_id == info.place_id) {
                let was = std::mem::replace(&mut existing.in_library, true);
                s.dirty = true;
                let id = existing.id;
                return (
                    id,
                    if was {
                        (Tone::Neutral, format!("{} is already in your library", info.name))
                    } else {
                        (Tone::Positive, format!("Added {} to your library", info.name))
                    },
                );
            }
            let id = Uuid::new_v4();
            s.bootstrapper.games.push(Game {
                id,
                place_id: info.place_id,
                universe_id: Some(info.universe_id),
                name: info.name.clone(),
                developer: info.developer.clone(),
                description: info.description.clone(),
                favorite: false,
                added_at: now,
                last_played: None,
                play_count: 0,
                config: LaunchConfig::default(),
                collection: None,
                in_library: true,
            });
            s.dirty = true;
            (id, (Tone::Positive, format!("Added {} to your library", info.name)))
        });
        self.toast(message.0, message.1);
        self.spawn(|s| async move { s.ensure_artwork().await });
        id
    }

    pub fn add_to_library(&self, id: Uuid) {
        let name = self.mutate(|s| {
            let now = s.now();
            let game = s.bootstrapper.game_mut(id)?;
            game.in_library = true;
            game.added_at = now;
            let name = game.name.clone();
            s.dirty = true;
            Some(name)
        });
        if let Some(name) = name {
            self.toast(Tone::Positive, format!("Added {name} to your library"));
        }
    }

    pub fn remove_game(&self, id: Uuid) {
        let removed = self.mutate(|s| {
            let game = s.bootstrapper.game(id)?;
            let (name, in_library) = (game.name.clone(), game.in_library);
            s.bootstrapper.games.retain(|g| g.id != id);
            s.bootstrapper.servers.retain(|g| g.game_id != id);
            s.bootstrapper.activity.retain(|a| a.game_id != id);
            s.dirty = true;
            Some((name, in_library))
        });
        if let Some((name, in_library)) = removed {
            self.toast(
                Tone::Neutral,
                if in_library { format!("Removed {name} from your library") } else { format!("Removed {name} from Recents") },
            );
        }
    }

    pub fn toggle_favorite(&self, id: Uuid) {
        self.mutate(|s| {
            if let Some(game) = s.bootstrapper.game_mut(id) {
                game.favorite = !game.favorite;
                // Favoriting a game you've only played adds it.
                if game.favorite {
                    game.in_library = true;
                }
                s.dirty = true;
            }
        });
    }

    pub fn set_collection(&self, id: Uuid, name: Option<String>) {
        let name = name.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty());
        self.mutate(|s| {
            if let Some(game) = s.bootstrapper.game_mut(id) {
                game.collection = name.clone();
                s.dirty = true;
            }
        });
        if let Some(name) = name {
            self.toast(Tone::Positive, format!("Added to {name}"));
        }
    }

    /// Saves what Play does for a game. An account here is pinned: it's used
    /// even when another account is picked in the sidebar.
    pub fn set_launch_config(&self, id: Uuid, account: Option<Uuid>, version: VersionChoice, server: ServerChoice) {
        self.mutate(|s| {
            if let Some(game) = s.bootstrapper.game_mut(id) {
                game.config = LaunchConfig {
                    account,
                    pin_account: account.is_some(),
                    version,
                    server,
                };
                s.dirty = true;
            }
        });
        self.toast(Tone::Positive, "Launch configuration saved");
    }

    pub async fn refresh_game(self: &Shared, id: Uuid, quiet: bool) {
        let Some((place, key)) = self.read().bootstrapper.game(id).map(|g| (g.place_id, g.artwork_key())) else { return };
        match roblox::fetch_game(place).await {
            Ok(info) => {
                self.mutate(|s| {
                    if let Some(game) = s.bootstrapper.game_mut(id) {
                        game.name = info.name;
                        game.developer = info.developer;
                        game.description = info.description;
                        game.universe_id = Some(info.universe_id);
                        s.dirty = true;
                    }
                });
                if !quiet {
                    self.toast(Tone::Positive, "Game details updated");
                    // A fresh thumbnail replaces the old one only once it's downloaded.
                    let universe = self.read().bootstrapper.game(id).and_then(|g| g.universe_id);
                    if let Some(universe) = universe {
                        if let Ok(bytes) = roblox::fetch_game_artwork(universe).await {
                            if let Some(path) = store::store_artwork(&key, &bytes).await {
                                self.mutate(|s| s.images.insert(key.clone(), path.display().to_string()));
                            }
                        }
                    }
                }
                self.ensure_artwork().await;
                self.sync_presence().await;
            }
            Err(error) if !quiet => self.toast(Tone::Negative, error),
            Err(_) => {}
        }
    }

    /// The game for a place: the library's own, or a new entry that only
    /// shows under Recents (its details are looked up in the background).
    pub fn game_for_place(self: &Shared, place_id: u64, name: Option<String>) -> Uuid {
        let (id, new) = self.mutate(|s| {
            if let Some(game) = s.bootstrapper.games.iter().find(|g| g.place_id == place_id) {
                return (game.id, false);
            }
            let game = Game::stub(place_id, name.unwrap_or_else(|| format!("Place {place_id}")), s.now());
            let id = game.id;
            s.bootstrapper.games.push(game);
            s.dirty = true;
            (id, true)
        });
        if new {
            self.spawn(move |s| async move { s.refresh_game(id, true).await });
        }
        id
    }

    // ── Private servers ──────────────────────────────────────────────────

    /// Saves a private server. Without a name, the server's own name is
    /// used (or whose server it is); without a game, the link's game.
    pub async fn save_server(self: &Shared, form: ServerForm) -> Result<(), String> {
        let link = form.link.trim().to_owned();
        if parse_server_link(&link).is_none() {
            return Err(
                "Paste a private server link (it contains privateServerLinkCode= or roblox.com/share?code=).".into(),
            );
        }
        let needs_info = form.name.trim().is_empty() || form.game.is_none();
        let info = if needs_info { self.server_link_info(link.clone()).await.ok() } else { None };
        let game = match (form.game, info.as_ref().and_then(|i| i.place_id)) {
            (Some(game), _) => game,
            (None, Some(place)) => self.game_for_place(place, info.as_ref().and_then(|i| i.game.clone())),
            (None, None) => return Err("Choose which game this server belongs to.".into()),
        };
        let name = match form.name.trim() {
            "" => info
                .as_ref()
                .and_then(|i| i.name.clone())
                .or_else(|| self.read().bootstrapper.game(game).map(|g| format!("{} private server", g.name)))
                .unwrap_or_else(|| "Private Server".to_owned()),
            n => n.to_owned(),
        };
        let server_id = Some(form.server_id.trim().to_owned()).filter(|s| !s.is_empty());
        let notes = form.notes.trim().to_owned();
        let editing = form.editing;
        self.mutate(|s| {
            let now = s.now();
            match editing.and_then(|id| s.bootstrapper.server_mut(id)) {
                Some(server) => {
                    server.game_id = game;
                    server.name = name.clone();
                    server.link = link;
                    server.server_id = server_id;
                    server.notes = notes;
                }
                None => s.bootstrapper.servers.push(PrivateServer {
                    id: Uuid::new_v4(),
                    game_id: game,
                    name: name.clone(),
                    link,
                    server_id,
                    notes,
                    favorite: false,
                    added_at: now,
                    last_joined: None,
                }),
            }
            s.dirty = true;
        });
        self.toast(
            Tone::Positive,
            if editing.is_some() { "Private server updated".to_owned() } else { format!("Saved {name}") },
        );
        Ok(())
    }

    pub fn remove_server(&self, id: Uuid) {
        self.mutate(|s| {
            s.bootstrapper.servers.retain(|x| x.id != id);
            for game in &mut s.bootstrapper.games {
                if game.config.server == ServerChoice::Private(id) {
                    game.config.server = ServerChoice::Public;
                }
            }
            s.dirty = true;
        });
        self.toast(Tone::Neutral, "Private server removed");
    }

    pub fn toggle_server_favorite(&self, id: Uuid) {
        self.mutate(|s| {
            if let Some(server) = s.bootstrapper.server_mut(id) {
                server.favorite = !server.favorite;
                s.dirty = true;
            }
        });
    }

    // ── Artwork ──────────────────────────────────────────────────────────

    /// Downloads any game thumbnails and avatars not cached yet. The window
    /// shows them straight from the cache folder.
    pub async fn ensure_artwork(self: &Shared) {
        let wanted: Vec<(String, Artwork)> = {
            let mut s = self.read();
            if !s.loaded {
                return;
            }
            let mut wanted = Vec::new();
            let games = s.bootstrapper.games.iter().filter_map(|g| Some((g.artwork_key(), Artwork::Game(g.universe_id?))));
            let recs = s.recommendations.iter().map(|r| (r.artwork_key(), Artwork::Game(r.universe_id)));
            let friends: Vec<(String, Artwork)> = s
                .friends
                .list
                .iter()
                .filter_map(|f| Some((format!("game-{}", f.place_id?), Artwork::Game(f.universe_id?))))
                .collect();
            let recs = recs.chain(friends);
            let avatars = s.bootstrapper.accounts.iter().map(|a| (a.avatar_key(), Artwork::Avatar(a.user_id)));
            let all: Vec<_> = games.chain(recs).chain(avatars).collect();
            for (key, kind) in all {
                if !s.images.contains_key(&key) && s.fetching.insert(key.clone()) {
                    wanted.push((key, kind));
                }
            }
            wanted
        };

        let downloads = wanted.into_iter().map(|(key, kind)| async move {
            let bytes = match kind {
                Artwork::Game(universe) => roblox::fetch_game_artwork(universe).await,
                Artwork::Avatar(user) => roblox::fetch_avatar(user).await,
            };
            let path = match bytes {
                Ok(bytes) => store::store_artwork(&key, &bytes).await,
                Err(_) => None,
            };
            (key, path)
        });
        for chunk in futures::future::join_all(downloads).await.chunks(8) {
            self.mutate(|s| {
                for (key, path) in chunk {
                    s.fetching.remove(key);
                    if let Some(path) = path {
                        s.images.insert(key.clone(), path.display().to_string());
                    }
                }
            });
        }
    }

    // ── Recommendations ──────────────────────────────────────────────────

    /// Seeds from the activity Pious has recorded for all accounts.
    fn local_seeds(&self) -> Vec<Seed> {
        let s = self.read();
        let library = &s.bootstrapper;
        let clock = s.now();
        let mut seeds: HashMap<u64, Seed> = HashMap::new();

        for entry in &library.activity {
            let Some(game) = library.game(entry.game_id) else { continue };
            let Some(universe) = game.universe_id else { continue };
            let days = (clock - entry.at).num_hours().max(0) as f32 / 24.0;
            let seed = seeds.entry(universe).or_insert_with(|| Seed {
                universe_id: universe,
                name: game.name.clone(),
                weight: 0.0,
                accounts: Vec::new(),
            });
            seed.weight += (-days / 21.0).exp();
            if let Some(account) = entry.account_id.and_then(|id| library.account(id)) {
                let label = account.label().to_owned();
                if !seed.accounts.contains(&label) {
                    seed.accounts.push(label);
                }
            }
        }

        let has_history = !seeds.is_empty();
        for game in library.library_games() {
            let Some(universe) = game.universe_id else { continue };
            let bonus = if game.favorite { 0.5 } else { 0.0 } + 0.04 * game.play_count as f32;
            match seeds.get_mut(&universe) {
                Some(seed) => seed.weight += bonus,
                // Without any history yet, the library itself is the signal.
                None if !has_history || game.favorite => {
                    seeds.insert(
                        universe,
                        Seed {
                            universe_id: universe,
                            name: game.name.clone(),
                            weight: 0.3 + bonus,
                            accounts: Vec::new(),
                        },
                    );
                }
                None => {}
            }
        }
        seeds.into_values().collect()
    }

    pub async fn refresh_recommendations(self: &Shared) {
        let seeds = self.local_seeds();
        let (accounts, exclude) = {
            let s = self.read();
            let accounts: Vec<(Uuid, String)> = s
                .bootstrapper
                .accounts
                .iter()
                .filter(|a| !a.needs_sign_in)
                .map(|a| (a.id, a.label().to_owned()))
                .collect();
            let exclude: HashSet<u64> = s.bootstrapper.games.iter().filter_map(|g| g.universe_id).collect();
            (accounts, exclude)
        };
        if seeds.is_empty() && accounts.is_empty() {
            self.mutate(|s| s.recommendations.clear());
            return;
        }
        self.mutate(|s| {
            s.recs_loading = true;
            s.recs_error = None;
        });

        let mut seeds: HashMap<u64, Seed> = seeds.into_iter().map(|s| (s.universe_id, s)).collect();
        // Add what each account has been playing on Roblox itself.
        for (id, label) in accounts {
            let token = tokio::task::spawn_blocking(move || credentials::load_session(id))
                .await
                .ok()
                .and_then(Result::ok);
            let Some(token) = token else { continue };
            let Ok(played) = roblox::recently_played(&token).await else { continue };
            for (rank, (universe, name)) in played.into_iter().take(10).enumerate() {
                let seed = seeds.entry(universe).or_insert_with(|| Seed {
                    universe_id: universe,
                    name: name.clone(),
                    weight: 0.0,
                    accounts: Vec::new(),
                });
                if seed.name.is_empty() {
                    seed.name = name;
                }
                seed.weight += 0.9 / (1.0 + 0.3 * rank as f32);
                if !seed.accounts.contains(&label) {
                    seed.accounts.push(label.clone());
                }
            }
        }

        let result = recommend::recommend(seeds.into_values().collect(), exclude, 60).await;
        self.mutate(|s| {
            s.recs_loading = false;
            match result {
                Ok(recs) => {
                    crate::core::cache::save("recommendations", &recs);
                    s.recommendations = recs;
                    s.recs_error = None;
                }
                Err(error) => s.recs_error = Some(error),
            }
        });
        self.ensure_artwork().await;
    }

    /// A recommendation as a game entry. With `add`, it's added to the
    /// library; otherwise it only shows under Recents once played.
    pub fn recommendation_game(self: &Shared, universe: u64, add: bool) -> Option<Uuid> {
        let (id, name) = self.mutate(|s| {
            let rec = s.recommendations.iter().find(|r| r.universe_id == universe)?.clone();
            if add {
                s.recommendations.retain(|r| r.universe_id != universe);
            }
            let now = s.now();
            let id = match s.bootstrapper.games.iter_mut().find(|g| g.place_id == rec.place_id) {
                Some(game) => {
                    game.in_library |= add;
                    game.id
                }
                None => {
                    let mut game = Game::stub(rec.place_id, rec.name.clone(), now);
                    game.universe_id = Some(rec.universe_id);
                    game.developer = (!rec.creator.is_empty()).then(|| rec.creator.clone());
                    game.in_library = add;
                    let id = game.id;
                    s.bootstrapper.games.push(game);
                    id
                }
            };
            s.dirty = true;
            Some((id, rec.name))
        })?;
        if add {
            self.toast(Tone::Positive, format!("Added {name} to your library"));
        }
        self.spawn(move |s| async move { s.refresh_game(id, true).await });
        Some(id)
    }
}

enum Artwork {
    Game(u64),
    Avatar(u64),
}

impl super::State {
    pub fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }
}
