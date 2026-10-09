//! Persistent domain types for Pious.
//!
//! Everything in here is plain data: it is serialized to disk by
//! [`crate::core::store`] and rendered by the UI layer, but carries no UI or
//! I/O concerns of its own.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The complete, persisted state of the user's library.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Bootstrapper {
    pub games: Vec<Game>,
    pub servers: Vec<PrivateServer>,
    pub accounts: Vec<Account>,
    pub versions: Vec<VersionRecord>,
    pub activity: Vec<Activity>,
    pub preferences: Preferences,
    /// The account the sidebar switcher picked for this session: what Play
    /// uses unless a game has an account pinned. Not saved; each session
    /// starts with the default account.
    #[serde(skip)]
    pub active_account: Option<Uuid>,
}

impl Bootstrapper {
    pub fn game(&self, id: Uuid) -> Option<&Game> {
        self.games.iter().find(|g| g.id == id)
    }

    pub fn game_mut(&mut self, id: Uuid) -> Option<&mut Game> {
        self.games.iter_mut().find(|g| g.id == id)
    }

    pub fn server(&self, id: Uuid) -> Option<&PrivateServer> {
        self.servers.iter().find(|s| s.id == id)
    }

    pub fn server_mut(&mut self, id: Uuid) -> Option<&mut PrivateServer> {
        self.servers.iter_mut().find(|s| s.id == id)
    }

    pub fn account(&self, id: Uuid) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }

    pub fn account_mut(&mut self, id: Uuid) -> Option<&mut Account> {
        self.accounts.iter_mut().find(|a| a.id == id)
    }

    pub fn version(&self, hash: &str) -> Option<&VersionRecord> {
        self.versions.iter().find(|v| v.hash == hash)
    }

    /// Games the user added to their library (not ones only played).
    pub fn library_games(&self) -> impl Iterator<Item = &Game> {
        self.games.iter().filter(|g| g.in_library)
    }


    pub fn default_account(&self) -> Option<&Account> {
        self.preferences
            .default_account
            .and_then(|id| self.account(id))
            .or_else(|| {
                self.accounts
                    .iter()
                    .max_by_key(|a| a.last_used.unwrap_or(a.added_at))
            })
    }

    /// The account Play uses: the one picked in the sidebar, or the default.
    pub fn play_account(&self) -> Option<&Account> {
        self.active_account
            .and_then(|id| self.account(id))
            .or_else(|| self.default_account())
    }

    /// The version that "Latest" resolves to: the newest usable installed version.
    pub fn latest_installed(&self) -> Option<&VersionRecord> {
        self.versions
            .iter()
            .filter(|v| v.is_usable())
            .max_by_key(|v| v.installed_at)
    }

    /// Resolves a [`VersionChoice`] to a concrete installed version, if any.
    pub fn resolve_version(&self, choice: &VersionChoice) -> Option<&VersionRecord> {
        match choice {
            VersionChoice::Default => match &self.preferences.default_version {
                VersionChoice::Specific(hash) => self
                    .version(hash)
                    .filter(|v| v.is_usable())
                    .or_else(|| self.latest_installed()),
                _ => self.latest_installed(),
            },
            VersionChoice::Latest => self.latest_installed(),
            VersionChoice::Specific(hash) => self.version(hash).filter(|v| v.is_usable()),
        }
    }

    /// Resolves the effective launch plan for a game, falling back to defaults.
    pub fn effective_config(&self, game: &Game) -> LaunchConfig {
        let mut config = game.config.clone();

        // Only an account the user pinned to this game overrides the one
        // picked in the sidebar.
        if !config.pin_account || config.account.is_none_or(|id| self.account(id).is_none()) {
            config.account = self.play_account().map(|a| a.id);
        }

        if let ServerChoice::Private(id) = config.server {
            if self.server(id).is_none() {
                config.server = ServerChoice::Public;
            }
        }

        config
    }


    pub fn record_activity(&mut self, activity: Activity) {
        self.activity.insert(0, activity);
        self.activity.truncate(200);
    }
}

/// A saved Roblox experience.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: Uuid,
    pub place_id: u64,
    #[serde(default)]
    pub universe_id: Option<u64>,
    pub name: String,
    #[serde(default)]
    pub developer: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub favorite: bool,
    pub added_at: DateTime<Utc>,
    #[serde(default)]
    pub last_played: Option<DateTime<Utc>>,
    #[serde(default)]
    pub play_count: u32,
    /// The remembered launch configuration for this game.
    #[serde(default)]
    pub config: LaunchConfig,
    #[serde(default)]
    pub collection: Option<String>,
    /// `false` for games that were only played (from recommendations, a
    /// joined player or a web launch) and not added to the library. They
    /// show up under Recents.
    #[serde(default = "yes")]
    pub in_library: bool,
}

fn yes() -> bool {
    true
}

impl Game {
    /// A game that hasn't been looked up yet.
    pub fn stub(place_id: u64, name: String, now: DateTime<Utc>) -> Self {
        Self {
            id: Uuid::new_v4(),
            place_id,
            universe_id: None,
            name,
            developer: None,
            description: None,
            favorite: false,
            added_at: now,
            last_played: None,
            play_count: 0,
            config: LaunchConfig::default(),
            collection: None,
            in_library: false,
        }
    }

    pub fn artwork_key(&self) -> String {
        format!("game-{}", self.place_id)
    }
}

/// A saved private (VIP) server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateServer {
    pub id: Uuid,
    pub game_id: Uuid,
    pub name: String,
    pub link: String,
    #[serde(default)]
    pub server_id: Option<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub favorite: bool,
    pub added_at: DateTime<Utc>,
    #[serde(default)]
    pub last_joined: Option<DateTime<Utc>>,
}

/// A locally saved Roblox account profile.
///
/// The session credential itself is never stored here: it lives in the
/// operating system's credential store, keyed by [`Account::id`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: Uuid,
    pub user_id: u64,
    pub username: String,
    pub display_name: String,
    /// Optional local nickname shown instead of the display name.
    #[serde(default)]
    pub alias: Option<String>,
    pub added_at: DateTime<Utc>,
    #[serde(default)]
    pub last_used: Option<DateTime<Utc>>,
    /// Set when Roblox reported the stored session as expired.
    #[serde(default)]
    pub needs_sign_in: bool,
    /// Keep its own copy of Roblox's local settings (volume, sensitivity,
    /// graphics…) and load it whenever this account plays.
    #[serde(default)]
    pub custom_settings: bool,
}

impl Account {
    pub fn label(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.display_name)
    }

    pub fn avatar_key(&self) -> String {
        format!("avatar-{}", self.user_id)
    }
}

/// Where a Roblox client build came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionSource {
    /// Installed by the official Roblox installer.
    Roblox,
    /// Installed by a third-party bootstrapper (e.g. Bloxstrap).
    Bootstrapper,
    /// Downloaded and managed by Pious.
    Pious,
}


/// A Roblox client build known to Pious.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionRecord {
    /// The build hash, e.g. `version-1a2b3c4d5e6f7a8b`.
    pub hash: String,
    /// Human-readable version, e.g. `0.695.0.6950667`, when known.
    #[serde(default)]
    pub version: Option<String>,
    pub path: std::path::PathBuf,
    pub source: VersionSource,
    pub installed_at: DateTime<Utc>,
    #[serde(default)]
    pub valid: bool,
    /// User-facing nickname, e.g. "Stable".
    #[serde(default)]
    pub label: Option<String>,
}

impl VersionRecord {
    /// Valid and its executable is there (from [`crate::core::fscache`], so
    /// this never waits on the disk while the app's state is locked).
    pub fn is_usable(&self) -> bool {
        self.valid && crate::core::fscache::is_file(&self.executable())
    }

    pub fn executable(&self) -> std::path::PathBuf {
        self.path.join("RobloxPlayerBeta.exe")
    }

    pub fn short_hash(&self) -> &str {
        let hash = self.hash.trim_start_matches("version-");
        &hash[..hash.len().min(8)]
    }

    pub fn title(&self) -> String {
        if let Some(label) = &self.label {
            return label.clone();
        }
        match &self.version {
            Some(v) => format!("Version {v}"),
            None => format!("Build {}", self.short_hash()),
        }
    }
}

/// Which client build a launch should use.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VersionChoice {
    /// Use the user's global default version.
    #[default]
    Default,
    /// Always use the newest installed build.
    Latest,
    /// Pin to a specific build hash.
    Specific(String),
}

/// Which server a launch should join.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServerChoice {
    #[default]
    Public,
    Private(Uuid),
}

/// A remembered combination of account, version and server.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LaunchConfig {
    pub account: Option<Uuid>,
    /// The account was chosen for this game on purpose, so it's used even
    /// when another account is picked in the sidebar.
    pub pin_account: bool,
    pub version: VersionChoice,
    pub server: ServerChoice,
}

/// Global user preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub default_account: Option<Uuid>,
    pub default_version: VersionChoice,
    /// Allow more than one Roblox client to run at the same time.
    pub multi_instance: bool,
    /// Remember the account/version/server used for each game automatically.
    pub remember_last_used: bool,
    /// Ask before closing a running instance.
    pub confirm_close: bool,
    /// Where Pious-managed versions are installed. `None` uses the default.
    pub versions_dir: Option<std::path::PathBuf>,
    /// Keep the window above other windows.
    pub pinned: bool,
    /// Show the sidebar as a narrow strip of icons.
    pub sidebar_collapsed: bool,
    /// Replace movement with instant changes.
    pub reduce_motion: bool,
    /// Look for a new release on GitHub when the app starts.
    pub auto_update: bool,
    /// Show accounts as a compact list instead of cards.
    pub accounts_list: bool,
    /// Show what you're playing on Discord.
    pub discord_presence: bool,
    /// Pious opens every Roblox launch, including Play on roblox.com.
    pub handle_roblox_links: bool,
    /// What the `roblox-player` and `roblox` links opened before Pious took
    /// them over, restored when the option is turned off.
    pub previous_handlers: Vec<(String, String)>,
    /// Start games through this bootstrapper (by name) instead of running
    /// the Roblox client directly, so its own mods and features apply.
    pub launch_via: Option<String>,
    /// Pious's own client tweaks (FastFlags, FPS cap, font, mods).
    pub tweaks: Tweaks,
    /// The in-game overlay opened with a hotkey.
    pub overlay: Overlay,
    pub anti_afk: AntiAfk,
    /// Rejoin automatically when a game disconnects you or crashes.
    pub auto_rejoin: bool,
    /// What the title bar's close button does.
    pub close_action: CloseAction,
    /// What Pious does when a game starts.
    pub on_game_launch: LaunchBehavior,
    /// Start Pious when Windows starts.
    pub start_with_windows: bool,
    /// When started with Windows, stay in the tray instead of opening.
    pub start_hidden: bool,
    /// Interface size, as a zoom factor.
    pub ui_scale: f32,
    /// How strongly the app blurs behind the search palette (0 = off).
    pub search_blur: f32,
    /// What Discord shows as the activity's name.
    pub discord_display: DiscordDisplay,
    /// Which Discord application the presence comes from: its name heads
    /// the card ("Roblox" or "Pious") and its icon shows when the game's
    /// picture can't.
    #[serde(default)]
    pub discord_app: DiscordApp,
    pub appearance: Appearance,
    /// Grid or list, per page ("games", "servers", "versions"…).
    pub views: std::collections::BTreeMap<String, ViewMode>,
    /// Which set of defaults these preferences were last brought up to
    /// (0 when saved before this existed).
    #[serde(default)]
    pub defaults_version: u32,
    /// Recording and clipping.
    pub recorder: Recorder,
    /// Shows only the start of every username (for streaming or recording).
    pub streamer_mode: bool,
    /// Where web links open.
    pub link_target: LinkTarget,
    /// What clicking the logo does: "home", "sidebar", "search",
    /// "play_last", "none", "page:<name>" or "macro:<id>".
    pub logo_action: String,
    /// Shortcuts inside Pious the user changed, by action ("" = off).
    pub shortcuts: std::collections::BTreeMap<String, String>,
    /// How each page is sorted, by page.
    pub sorts: std::collections::BTreeMap<String, String>,
    /// The welcome tour was shown.
    pub onboarded: bool,
    /// Tips that were dismissed.
    pub seen_tips: std::collections::BTreeSet<String>,
    /// The last version whose changes were shown.
    pub seen_version: String,
    /// Macros from before they moved into the Macros plugin. Read once, to
    /// move them into the plugin's data (see `plugins::migrate_legacy_macros`).
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub macros: serde_json::Value,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub macro_settings: serde_json::Value,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub autoclicker: serde_json::Value,
    /// Plugins that are turned on, by ID.
    pub plugins: std::collections::BTreeSet<String>,
    /// Lets AI apps control Pious through the Model Context Protocol.
    pub mcp: Mcp,
    /// Discord shows a "Join" button others can use to join your server.
    pub discord_join: bool,
    /// Show Roblox Studio on Discord while it's open.
    pub studio_presence: bool,
    /// Show "Playing Pious" on Discord while no game is open.
    pub pious_presence: bool,
    /// Show the Roblox account you're playing as (its avatar and name) on
    /// Discord.
    pub discord_account: bool,
    /// Pious's own pop-ups for Roblox messages and notifications.
    pub notifications: Notifications,
    /// Tell Roblox you're online while Pious is open, so friends see you
    /// as online (like the Roblox app does).
    pub appear_online: bool,
    /// Turn streamer mode on by itself while streaming software is open.
    pub streamer_auto: bool,
    /// The programs that count as streaming software (process names).
    pub streamer_apps: Vec<String>,
    /// Roblox, Byfron and Hyperion news on the home page.
    pub news_on_home: bool,
    /// The Friends page lists the friends of every account together.
    pub friends_all: bool,
    /// The DNS servers Roblox's lookups go to: "auto" (the network's), a
    /// provider (see `core::dns`) or "custom" for `dns_custom`.
    pub dns: String,
    pub dns_custom: Vec<String>,
    /// Say where the server you joined is (and show it in Instances).
    pub server_location: bool,
    /// Keys and mouse buttons drawn over the game, like streamers use.
    pub input_overlay: InputOverlay,
    /// FPS, ping, players and more drawn over the game.
    #[serde(default)]
    pub stats_overlay: StatsOverlay,
    /// Keys and buttons that press something else in Roblox.
    pub keybinds: Keybinds,
    /// Discord-style :shortcodes: that turn into emoji.
    pub emoji_shortcodes: EmojiShortcodes,
    /// Windows taskbar: flashing and badges.
    pub taskbar: Taskbar,
    /// Keep Roblox up to date in the background.
    pub auto_update_roblox: bool,
    /// Keep crash reports (Pious's and Roblox's) in the data folder.
    pub crash_reports: bool,
    /// Where public servers are joined: "auto" (Roblox's matchmaking),
    /// "best_ping" or a region ID (see `core::region`).
    pub region: String,
    /// How Auto arrange lays out Roblox windows: "grid", "columns", "rows"
    /// or "cascade".
    pub arrange_layout: String,
}

/// What Pious does with its taskbar button.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Taskbar {
    /// Flash the taskbar button when something needs attention while Pious
    /// isn't in front (a message, a finished download).
    pub flash: bool,
    /// A count badge on the taskbar button for unread pop-ups.
    pub badge: bool,
    /// Download progress on the taskbar button.
    pub progress: bool,
}

impl Default for Taskbar {
    fn default() -> Self {
        Self { flash: true, badge: true, progress: true }
    }
}

/// Streaming and recording programs that turn streamer mode on.
pub fn default_streamer_apps() -> Vec<String> {
    [
        "obs64.exe",
        "obs32.exe",
        "obs.exe",
        "Streamlabs OBS.exe",
        "Streamlabs Desktop.exe",
        "XSplit.Core.exe",
        "TwitchStudio.exe",
        "PRISMLiveStudio.exe",
        "vMix64.exe",
        "Meld Studio.exe",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EmojiShortcodes {
    /// In Pious's chat boxes.
    pub in_app: bool,
    /// While typing in Roblox (watches the keyboard while Roblox is in front).
    pub in_roblox: bool,
}

impl Default for EmojiShortcodes {
    fn default() -> Self {
        Self { in_app: true, in_roblox: false }
    }
}

/// Live numbers about the game drawn over it (frame rate, the server's
/// ping and players, where it is…), like the input overlay but for stats.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatsOverlay {
    pub enabled: bool,
    /// What it shows, in this order: "fps", "ping", "players", "server_fps",
    /// "location", "session", "cpu", "memory", "clock".
    pub items: Vec<String>,
    /// "row" (side by side) or "column".
    pub layout: String,
    /// Where it sits on the game's screen, as fractions (0–1) of the screen.
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    /// Colors as #RRGGBB or #RRGGBBAA.
    pub background: String,
    pub text_color: String,
    pub label_color: String,
    pub font: String,
    pub opacity: f32,
    /// Short labels ("FPS") instead of none.
    pub labels: bool,
    pub only_in_game: bool,
    pub show_in_recordings: bool,
}

impl Default for StatsOverlay {
    fn default() -> Self {
        Self {
            enabled: false,
            items: ["fps", "ping", "players", "location", "session"].map(String::from).to_vec(),
            layout: "row".into(),
            x: 0.01,
            y: 0.01,
            scale: 1.0,
            background: "#000000A6".into(),
            text_color: "#FFFFFF".into(),
            label_color: "#FFFFFFA0".into(),
            font: "Manrope".into(),
            opacity: 1.0,
            labels: true,
            only_in_game: true,
            show_in_recordings: false,
        }
    }
}

/// The on-screen keys and mouse (an input overlay).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InputOverlay {
    pub enabled: bool,
    /// "streamer", "wasd", "fps", "keyboard", "mouse", "none" or "custom".
    pub layout: String,
    /// "slanted" (keys leaning, like streamers' overlays) or "flat".
    pub style: String,
    /// How far slanted keys lean, in degrees.
    pub slant: f32,
    /// The custom layout: rows of key names ("KeyW", "Space", "MouseLeft"…).
    pub rows: Vec<Vec<String>>,
    pub show_mouse: bool,
    pub show_scroll: bool,
    /// Clicks and keys per second.
    pub show_rates: bool,
    /// The game's frame rate.
    pub show_fps: bool,
    /// Where it sits on the game's screen, as fractions (0–1) of the screen.
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    /// Size of one key, in pixels before scaling.
    pub key_size: u32,
    pub gap: u32,
    pub radius: u32,
    pub border: u32,
    /// Colors as #RRGGBB or #RRGGBBAA.
    pub background: String,
    pub key_color: String,
    pub pressed_color: String,
    pub text_color: String,
    pub pressed_text_color: String,
    pub border_color: String,
    pub font: String,
    pub font_size: u32,
    pub bold: bool,
    pub uppercase: bool,
    /// "none", "pop", "glow" or "fade".
    pub animation: String,
    /// How long a key takes to fade back after it's let go (ms).
    pub release_ms: u32,
    pub opacity: f32,
    /// Only while a Roblox window is in front.
    pub only_in_game: bool,
    /// Only while Pious is recording or keeping clips, so it's in videos
    /// but not in the way otherwise.
    pub only_while_capturing: bool,
    /// Shows up in Pious's recordings and clips (and in OBS's screen
    /// capture). Off keeps it out of every capture.
    pub show_in_recordings: bool,
}

impl Default for InputOverlay {
    fn default() -> Self {
        Self {
            enabled: false,
            layout: "streamer".into(),
            style: "slanted".into(),
            slant: 9.0,
            rows: vec![
                vec!["KeyQ".into(), "KeyW".into(), "KeyE".into(), "KeyR".into()],
                vec!["KeyA".into(), "KeyS".into(), "KeyD".into(), "KeyF".into()],
                vec!["ShiftLeft".into(), "Space".into()],
            ],
            show_mouse: true,
            show_scroll: true,
            show_rates: true,
            show_fps: false,
            x: 0.02,
            y: 0.68,
            scale: 1.0,
            key_size: 44,
            gap: 6,
            radius: 7,
            border: 2,
            background: "#00000000".into(),
            key_color: "#0000008C".into(),
            pressed_color: "#FFFFFFF2".into(),
            text_color: "#FFFFFFFF".into(),
            pressed_text_color: "#000000FF".into(),
            border_color: "#FFFFFFE6".into(),
            font: "Manrope".into(),
            font_size: 14,
            bold: true,
            uppercase: true,
            animation: "pop".into(),
            release_ms: 120,
            opacity: 1.0,
            only_in_game: true,
            only_while_capturing: false,
            show_in_recordings: true,
        }
    }
}

/// Remapped keys and buttons for Roblox.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Keybinds {
    pub enabled: bool,
    pub sets: Vec<KeybindSet>,
}

/// Remaps that apply everywhere in Roblox, to one account, or to one game.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeybindSet {
    pub id: Uuid,
    pub scope: KeybindScope,
    #[serde(default = "yes")]
    pub enabled: bool,
    pub binds: Vec<Keybind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id")]
pub enum KeybindScope {
    /// Every Roblox game.
    Global,
    /// One of the user's accounts.
    Account(Uuid),
    /// One game, by place ID.
    Game(u64),
}

/// Pressing `from` presses `to` instead ("" blocks the key).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keybind {
    pub from: String,
    pub to: String,
}

impl Keybinds {
    /// The remaps for a window playing `place` as `account` (either may be
    /// unknown): global ones, then the account's, then the game's, each
    /// winning over the ones before.
    pub fn resolve(&self, account: Option<Uuid>, place: Option<u64>) -> Vec<(String, String)> {
        let rank = |scope: &KeybindScope| match scope {
            KeybindScope::Global => Some(0),
            KeybindScope::Account(a) => (Some(*a) == account).then_some(1),
            KeybindScope::Game(p) => (Some(*p) == place).then_some(2),
        };
        let mut sets: Vec<(u8, &KeybindSet)> =
            self.sets.iter().filter(|s| s.enabled).filter_map(|s| rank(&s.scope).map(|r| (r, s))).collect();
        sets.sort_by_key(|(r, _)| *r);
        let mut out: Vec<(String, String)> = Vec::new();
        for bind in sets.iter().flat_map(|(_, set)| &set.binds).filter(|b| !b.from.is_empty()) {
            out.retain(|(from, _)| *from != bind.from);
            out.push((bind.from.clone(), bind.to.clone()));
        }
        out
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkTarget {
    /// A Pious window.
    #[default]
    Pious,
    /// The default web browser.
    Browser,
}

/// The local MCP server AI apps connect to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mcp {
    pub enabled: bool,
    pub port: u16,
    /// Let AI apps launch games, run macros and change things (not just look).
    pub allow_actions: bool,
    /// Let AI apps take pictures of Roblox windows and listen to them.
    pub allow_senses: bool,
    /// Let an AI app start Pious (in the tray) when it calls a tool while
    /// Pious is closed. Off: AI apps never start Pious by themselves.
    pub start_on_demand: bool,
}

impl Default for Mcp {
    fn default() -> Self {
        Self { enabled: false, port: 47_823, allow_actions: true, allow_senses: true, start_on_demand: false }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewMode {
    #[default]
    Grid,
    List,
}

/// Pious's own game recorder (it can take over Roblox's F12) and clipping
/// of the last few seconds from a rolling buffer. Both share one capture:
/// the screen is grabbed and encoded on the GPU into short segments, and
/// recordings and clips are cut from those without re-encoding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Recorder {
    /// The record hotkey starts and stops recordings.
    pub recording: bool,
    pub record_hotkey: String,
    /// Keep a rolling buffer while a game runs, so clips can be saved.
    pub clips: bool,
    /// Saves the last `clip_seconds`.
    pub clip_hotkey: String,
    pub clip_seconds: u32,
    /// Asks how many seconds to save, counted back from the key press.
    pub manual_clip_hotkey: String,
    /// How far back clips can reach (the buffer's length).
    pub buffer_seconds: u32,
    /// Hotkeys only work while a Roblox window (or Pious's overlay) is in
    /// front, so they don't take keys away from other apps.
    pub game_only: bool,
    /// Where videos go; `None` = Videos\Pious.
    pub folder: Option<std::path::PathBuf>,
    /// File name pattern: {game}, {date}, {time}, {kind}.
    pub file_name: String,
    pub container: Container,
    pub target: CaptureTarget,
    /// An FFmpeg encoder name, or "auto" for the best one this PC has.
    pub encoder: String,
    pub codec: VideoCodec,
    pub rate_control: RateControl,
    /// Constant-quality level (lower = better, bigger): 0–51.
    pub quality: u32,
    pub bitrate_kbps: u32,
    pub max_bitrate_kbps: u32,
    pub fps: u32,
    /// Output height; `None` keeps the game's resolution.
    pub height: Option<u32>,
    pub keyframe_seconds: f32,
    pub speed: EncodeSpeed,
    pub ten_bit: bool,
    pub cursor: bool,
    pub game_audio: GameAudio,
    pub game_volume: f32,
    pub mic: bool,
    /// A microphone's device ID; `None` = Windows' default.
    pub mic_device: Option<String>,
    pub mic_volume: f32,
    /// Game and microphone on their own audio tracks (for editing).
    pub separate_tracks: bool,
    pub audio_codec: AudioCodec,
    pub audio_bitrate_kbps: u32,
    /// Show a small notice over the game when a clip or recording is saved.
    pub notify: bool,
}

impl Default for Recorder {
    fn default() -> Self {
        Self {
            recording: false,
            record_hotkey: "F12".into(),
            clips: false,
            clip_hotkey: "F8".into(),
            clip_seconds: 30,
            manual_clip_hotkey: "Shift+F8".into(),
            buffer_seconds: 120,
            game_only: true,
            folder: None,
            file_name: "{game} {date} {time}".into(),
            // MOV and MP4 save the same way (a copy of what's recorded, no
            // re-encoding), so neither is faster; MOV is the default.
            container: Container::Mov,
            target: CaptureTarget::GameWindow,
            encoder: "auto".into(),
            codec: VideoCodec::H264,
            rate_control: RateControl::Quality,
            quality: 23,
            bitrate_kbps: 20_000,
            max_bitrate_kbps: 40_000,
            fps: 60,
            height: None,
            keyframe_seconds: 1.0,
            speed: EncodeSpeed::Balanced,
            ten_bit: false,
            cursor: true,
            game_audio: GameAudio::GameOnly,
            game_volume: 1.0,
            mic: false,
            mic_device: None,
            mic_volume: 1.0,
            separate_tracks: false,
            audio_codec: AudioCodec::Aac,
            audio_bitrate_kbps: 192,
            notify: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Container {
    #[default]
    Mp4,
    Mkv,
    Mov,
}

impl Container {
    pub fn extension(self) -> &'static str {
        match self {
            Container::Mp4 => "mp4",
            Container::Mkv => "mkv",
            Container::Mov => "mov",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureTarget {
    /// Just the Roblox window.
    #[default]
    GameWindow,
    /// The whole screen the game is on.
    GameMonitor,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoCodec {
    #[default]
    H264,
    Hevc,
    Av1,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RateControl {
    /// Constant quality (CQ/CRF): size follows what's on screen.
    #[default]
    Quality,
    /// Variable bitrate around a target, capped at a maximum.
    Vbr,
    /// Constant bitrate (for strict size limits).
    Cbr,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncodeSpeed {
    Fastest,
    Fast,
    #[default]
    Balanced,
    Quality,
    Best,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameAudio {
    /// Only Roblox's sound (Windows 10 2004 or newer).
    #[default]
    GameOnly,
    /// Everything your PC plays.
    Everything,
    Off,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioCodec {
    #[default]
    Aac,
    Opus,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloseAction {
    Quit,
    /// Keep running in the system tray.
    #[default]
    Tray,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LaunchBehavior {
    #[default]
    Stay,
    Minimize,
    /// Hide to the tray and come back when every Roblox window has closed.
    HideUntilClosed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscordApp {
    /// A Discord application named "Roblox", with Roblox's icon.
    #[default]
    Roblox,
    /// Pious's own application.
    Pious,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscordDisplay {
    /// "Playing <game name>".
    #[default]
    GameName,
    /// "Playing Pious". Older versions could show Roblox's or your own
    /// application's name instead.
    #[serde(alias = "Roblox", alias = "CustomApp")]
    Pious,
}

/// A named set of FastFlags.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlagProfile {
    pub id: Uuid,
    pub name: String,
    pub flags: std::collections::BTreeMap<String, String>,
}

/// Bootstrapper-style changes Pious applies to the builds it launches. They
/// are undone exactly (originals are backed up) when turned off.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tweaks {
    pub enabled: bool,
    /// Frame rate cap; `None` keeps Roblox's own.
    pub fps_limit: Option<u32>,
    pub graphics: GraphicsApi,
    /// Forced anti-aliasing samples (0 = automatic).
    pub msaa: u8,
    /// Forced texture quality 0–3; `None` = automatic.
    pub texture_quality: Option<u8>,
    /// Older free-form FastFlags (moved into a profile on load).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub custom_flags: String,
    /// Saved FastFlag sets; the active one is applied.
    pub flag_profiles: Vec<FlagProfile>,
    pub active_profile: Option<Uuid>,
    /// A font file that replaces Roblox's fonts.
    pub font: Option<std::path::PathBuf>,
    /// A ready-made font instead of a file: "roblox:<file>" for one of
    /// Roblox's own fonts, or a preset ID (see `core::fonts`).
    pub font_preset: Option<String>,
    /// Copy the files in Pious's Modifications folder over the client.
    pub use_mods_folder: bool,
    /// Ignore Windows display scaling (sharper on high-DPI screens).
    pub disable_dpi_scaling: bool,
    /// Let Alt+Enter switch fullscreen.
    pub alt_enter_fullscreen: bool,
    /// Roblox's graphics quality 1–10 (its own setting, in its settings
    /// file); `None` leaves Roblox's.
    pub graphics_quality: Option<u8>,
    /// Stop lighting and shadows from updating (big frame gain, flat look).
    pub pause_voxelizer: bool,
    /// A plain gray sky instead of the game's.
    pub gray_sky: bool,
    /// Grass that doesn't sway.
    pub still_grass: bool,
    /// No grass at all (a real frame gain on terrain-heavy games).
    pub no_grass: bool,
    /// Mesh detail 0 (lowest) – 4 (highest); `None` = automatic.
    pub mesh_detail: Option<u8>,
    /// The classic mouse cursor.
    pub cursor: CursorStyle,
    /// The old walk, jump and get-up sounds.
    pub old_character_sounds: bool,
    /// The old avatar editor background.
    pub old_avatar_background: bool,
    /// The emoji font.
    pub emoji: EmojiStyle,
    /// Text for the Roblox window's title bar (empty = "Roblox").
    pub window_title: String,
    /// The shift lock cursor: a preset (see `core::crosshair`), "custom"
    /// for `shiftlock_file`, or `None` for Roblox's own.
    pub shiftlock: Option<String>,
    /// The preset's color (`#RRGGBB`), or its own.
    pub shiftlock_color: Option<String>,
    pub shiftlock_file: Option<std::path::PathBuf>,
    /// The Roblox window's icon: a preset (see `core::playericon`),
    /// "custom" for `player_icon_file`, or `None` for Roblox's own.
    pub player_icon: Option<String>,
    pub player_icon_file: Option<std::path::PathBuf>,
    /// Delete Roblox logs and cache older than this many days.
    pub cleaner_days: Option<u32>,
    /// Windows compatibility: turn off fullscreen optimizations for Roblox.
    #[serde(default)]
    pub disable_fullscreen_optimizations: bool,
    /// Windows compatibility: who scales Roblox on high-DPI screens
    /// ("application", "system" or "system_enhanced"; `None` = Windows).
    #[serde(default)]
    pub dpi_override: Option<String>,
    /// Roblox's CPU priority ("above_normal" or "high"; `None` = normal).
    #[serde(default)]
    pub priority: Option<String>,
    /// Anti-aliasing off (×0). Separate from `msaa`, where 0 means "Roblox
    /// decides".
    #[serde(default)]
    pub msaa_off: bool,
    /// Roblox's finer rendering quality, 1–21 (the old hidden setting the
    /// 10-step slider is mapped onto); `None` leaves it to the slider.
    #[serde(default)]
    pub render_quality: Option<u8>,
    /// The sky: a preset (see `core::skybox`), "custom" for `skybox_folder`,
    /// or `None` for Roblox's own. Games with their own sky keep it.
    #[serde(default)]
    pub skybox: Option<String>,
    #[serde(default)]
    pub skybox_folder: Option<std::path::PathBuf>,
    /// Which graphics card Windows runs Roblox on (Settings → System →
    /// Display → Graphics): "default" (let Windows decide), "power_saving",
    /// "high_performance", or "adapter:<n>" for a specific card; `None`
    /// leaves Windows' setting alone.
    #[serde(default)]
    pub gpu: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorStyle {
    #[default]
    Default,
    From2006,
    From2013,
    // From Voidstrap's cursor set.
    BibataModernIce,
    Clean,
    Dot,
    Fps,
    Stoofs,
    VerySmallWhiteDot,
    WhiteDot,
    // From Froststrap's cursor set.
    BlackAndWhiteDot,
    PurpleCross,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmojiStyle {
    #[default]
    Default,
    /// Apple's (built on this PC from Apple's emoji pictures).
    Apple,
    /// Windows 11 23H2 and later (Fluent).
    Windows11Fluent,
    /// Windows 11 (22H2).
    Windows11,
    /// Windows 11 (original).
    Windows11Original,
    Windows10,
    /// Windows 10 Anniversary Update.
    Windows10Anniversary,
    Windows8,
    /// Twitter's SVG emoji (sharper Twemoji).
    TwemojiSvg,
    EmojiOne,
    Catmoji,
    /// Noto, Google's black-and-white set.
    NotoMono,
    /// OpenMoji, black-and-white.
    OpenMojiMono,
    /// Mona 12, colored pixel emoji.
    Mona12,
    /// The original 2000s Japanese phone emoji (DoCoMo).
    Docomo,
}

impl Tweaks {
    /// Moves the old free-form flags into a profile.
    pub fn migrate(&mut self) {
        let text = std::mem::take(&mut self.custom_flags);
        let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(text.trim()) else {
            return;
        };
        let flags = map
            .into_iter()
            .map(|(k, v)| (k, v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string())))
            .collect();
        let id = Uuid::new_v4();
        self.flag_profiles.push(FlagProfile { id, name: "My flags".into(), flags });
        self.active_profile.get_or_insert(id);
    }

    pub fn active_flags(&self) -> Option<&FlagProfile> {
        let id = self.active_profile?;
        self.flag_profiles.iter().find(|p| p.id == id)
    }

    /// Every tweak back to Roblox's normal behavior. Saved FastFlag
    /// profiles are kept (they're the user's work), just not used; the
    /// master switch stays as it is.
    pub fn reset(&mut self) {
        let keep = (self.enabled, std::mem::take(&mut self.flag_profiles));
        *self = Tweaks { enabled: keep.0, flag_profiles: keep.1, ..Tweaks::default() };
    }

    /// Whether every tweak is at its normal value (so having tweaks on
    /// changes nothing).
    pub fn is_neutral(&self) -> bool {
        let mut plain = self.clone();
        plain.enabled = Tweaks::default().enabled;
        plain.flag_profiles = Vec::new();
        plain.custom_flags.clear();
        plain == Tweaks::default()
    }
}

impl Default for Tweaks {
    fn default() -> Self {
        Self {
            // On, with every tweak at its normal value: nothing changes until
            // the user changes a tweak.
            enabled: true,
            fps_limit: None,
            graphics: GraphicsApi::Automatic,
            msaa: 0,
            texture_quality: None,
            custom_flags: String::new(),
            flag_profiles: Vec::new(),
            active_profile: None,
            font: None,
            font_preset: None,
            use_mods_folder: false,
            disable_dpi_scaling: false,
            alt_enter_fullscreen: false,
            graphics_quality: None,
            pause_voxelizer: false,
            gray_sky: false,
            still_grass: false,
            no_grass: false,
            mesh_detail: None,
            cursor: CursorStyle::Default,
            old_character_sounds: false,
            old_avatar_background: false,
            emoji: EmojiStyle::Default,
            window_title: String::new(),
            shiftlock: None,
            shiftlock_color: None,
            shiftlock_file: None,
            player_icon: None,
            player_icon_file: None,
            cleaner_days: None,
            disable_fullscreen_optimizations: false,
            dpi_override: None,
            priority: None,
            msaa_off: false,
            render_quality: None,
            skybox: None,
            skybox_folder: None,
            gpu: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphicsApi {
    #[default]
    Automatic,
    Direct3D11,
    Vulkan,
    OpenGL,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Overlay {
    pub enabled: bool,
    /// The key combination that opens it, e.g. `Alt+Backquote`.
    pub hotkey: String,
    pub blur: OverlayBlur,
    /// Blur strength for the adjustable blur (0–1).
    pub blur_strength: f32,
    /// How dark the game gets behind the overlay (0–1).
    pub dim: f32,
    /// Where each panel sits, by panel name.
    pub widgets: std::collections::BTreeMap<String, Widget>,
    /// The hotkey only works while a Roblox window is in front (so a key
    /// like Home keeps working everywhere else).
    pub game_only: bool,
    /// A panel for what's playing on the PC (Spotify, YouTube…).
    pub media: bool,
}

impl Default for Overlay {
    fn default() -> Self {
        Self {
            enabled: true,
            hotkey: "Home".into(),
            blur: OverlayBlur::Adjustable,
            // Light by default: a heavy blur costs more to draw and hides
            // more of the game.
            blur_strength: 0.25,
            dim: 0.35,
            widgets: Default::default(),
            game_only: true,
            media: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverlayBlur {
    Off,
    /// Windows' own live blur of the game (fixed strength).
    Live,
    /// A blurred still of the game, any strength.
    #[default]
    Adjustable,
}

/// Pop-ups in the corner of the screen for what happens on Roblox.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Notifications {
    pub enabled: bool,
    /// A friend messaged you.
    pub messages: bool,
    pub friend_requests: bool,
    /// A friend started playing a game.
    pub friend_joins: bool,
    /// Roblox's own notifications (the bell on the website).
    pub roblox: bool,
    /// Also while a game is in front (over the game).
    pub in_game: bool,
    /// A short sound with each one.
    pub sound: bool,
    /// How long one stays, in seconds.
    pub seconds: u32,
    /// "rich": each kind its own look (a game's banner, a chat bubble);
    /// "compact": one small card for everything.
    pub style: String,
}

impl Default for Notifications {
    fn default() -> Self {
        Self {
            enabled: true,
            messages: true,
            friend_requests: true,
            friend_joins: false,
            roblox: true,
            in_game: true,
            sound: true,
            seconds: 6,
            style: "rich".into(),
        }
    }
}

/// A movable overlay panel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Widget {
    /// Position as a fraction of the screen (0–1), so it survives
    /// resolution changes.
    pub x: f32,
    pub y: f32,
    pub pinned: bool,
    #[serde(default)]
    pub hidden: bool,
    /// Size in pixels once resized; its natural size until then.
    #[serde(default)]
    pub width: Option<f32>,
    #[serde(default)]
    pub height: Option<f32>,
}

/// Keeps Roblox from disconnecting idle players (after 20 minutes).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AntiAfk {
    pub enabled: bool,
    /// How often each idle window gets a nudge.
    pub minutes: u32,
    pub action: AfkAction,
}

impl Default for AntiAfk {
    fn default() -> Self {
        Self {
            enabled: false,
            minutes: 9,
            action: AfkAction::Walk,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AfkAction {
    /// Space: a jump.
    Jump,
    /// W then S: a step forward and back.
    #[default]
    Walk,
    /// I then O: zoom the camera in and out.
    #[serde(alias = "Mouse")]
    Zoom,
}

/// User-customizable look of the application.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// Hex colors (`#RRGGBB`).
    pub accent: String,
    pub background: String,
    pub surface: String,
    pub text: String,
    /// Optional picture shown behind the interface.
    pub background_image: Option<std::path::PathBuf>,
    /// How strongly the background picture is dimmed (0 = not at all).
    pub image_dim: f32,
    /// How much the background picture is blurred (0 = sharp, 1 = very soft).
    pub image_blur: f32,
    /// Let the desktop show through the window.
    pub see_through: bool,
    /// Opacity of the window background while see-through is on.
    pub window_opacity: f32,
    /// Strength of the glass surfaces (0.5 = subtle, 2.0 = strong).
    pub glass: f32,
    /// How the desktop behind a see-through window is blurred.
    pub blur: Blur,
    /// How frosted the see-through blur looks (0 = light, 1 = heavy).
    pub blur_strength: f32,
    /// The theme these colors came from (a folder in `themes`, or a theme a
    /// plugin brings). Its extras (CSS, icons, font file) apply while set.
    pub theme: Option<String>,
    /// A gradient over the background.
    pub gradient: Gradient,
    /// The interface font: "" for Pious's own (Manrope), or any font
    /// installed on the PC, by family name.
    pub font: String,
    /// How round corners are (1 = Pious's own, 0 = square).
    pub radius: f32,
    /// Text size, as a factor of the normal size.
    pub font_scale: f32,
    /// A second accent for highlights and gradients (empty = the accent).
    pub accent_2: String,
}

/// A linear gradient across the window's background.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Gradient {
    pub enabled: bool,
    pub from: String,
    pub to: String,
    /// Degrees, like CSS (0 = upwards, 90 = to the right).
    pub angle: f32,
    /// How strongly it shows (0–1).
    pub opacity: f32,
}

impl Default for Gradient {
    fn default() -> Self {
        Self { enabled: false, from: "#2B1F4A".into(), to: "#0A0A0B".into(), angle: 135.0, opacity: 0.6 }
    }
}

/// Background blur for the see-through window, provided by the OS.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Blur {
    /// Clear glass: the desktop shows through sharply.
    Off,
    /// Frosted glass: what's behind the window is blurred (Windows acrylic).
    #[default]
    Frosted,
    /// A heavily diffused tint of the wallpaper (Windows 11 Mica).
    Diffused,
    /// What's behind the window, blurred by Pious at any strength.
    Adjustable,
}


impl Default for Appearance {
    fn default() -> Self {
        Self {
            accent: "#F2F2F3".into(),
            background: "#0A0A0B".into(),
            surface: "#FFFFFF".into(),
            text: "#EDEDEF".into(),
            background_image: None,
            image_dim: 0.6,
            image_blur: 0.0,
            see_through: true,
            window_opacity: 0.72,
            glass: 1.0,
            blur: Blur::Frosted,
            blur_strength: 0.5,
            theme: None,
            gradient: Gradient::default(),
            font: String::new(),
            radius: 1.0,
            font_scale: 1.0,
            accent_2: String::new(),
        }
    }
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            default_account: None,
            default_version: VersionChoice::Latest,
            multi_instance: true,
            remember_last_used: true,
            confirm_close: true,
            versions_dir: None,
            pinned: false,
            sidebar_collapsed: false,
            reduce_motion: false,
            auto_update: true,
            accounts_list: false,
            discord_presence: false,
            handle_roblox_links: false,
            previous_handlers: Vec::new(),
            launch_via: None,
            tweaks: Tweaks::default(),
            overlay: Overlay::default(),
            anti_afk: AntiAfk::default(),
            auto_rejoin: false,
            close_action: CloseAction::Tray,
            on_game_launch: LaunchBehavior::Stay,
            start_with_windows: false,
            start_hidden: true,
            ui_scale: 1.0,
            search_blur: 0.5,
            discord_display: DiscordDisplay::GameName,
            discord_app: DiscordApp::Roblox,
            appearance: Appearance::default(),
            views: [("versions".to_owned(), ViewMode::List)].into(),
            recorder: Recorder::default(),
            defaults_version: DEFAULTS_VERSION,
            streamer_mode: false,
            link_target: LinkTarget::Pious,
            logo_action: "home".into(),
            shortcuts: Default::default(),
            sorts: Default::default(),
            onboarded: false,
            seen_tips: Default::default(),
            seen_version: String::new(),
            macros: serde_json::Value::Null,
            macro_settings: serde_json::Value::Null,
            autoclicker: serde_json::Value::Null,
            plugins: Default::default(),
            mcp: Mcp::default(),
            discord_join: true,
            studio_presence: false,
            pious_presence: true,
            discord_account: false,
            notifications: Notifications::default(),
            appear_online: true,
            streamer_auto: false,
            streamer_apps: default_streamer_apps(),
            news_on_home: true,
            friends_all: false,
            dns: "auto".into(),
            dns_custom: Vec::new(),
            server_location: true,
            input_overlay: InputOverlay::default(),
            stats_overlay: StatsOverlay::default(),
            keybinds: Keybinds::default(),
            emoji_shortcodes: EmojiShortcodes::default(),
            taskbar: Taskbar::default(),
            auto_update_roblox: false,
            crash_reports: true,
            region: "auto".into(),
            arrange_layout: "grid".into(),
        }
    }
}

/// Bumped when defaults change; settings still at an old default move to
/// the new one (ones the user changed stay as they are).
const DEFAULTS_VERSION: u32 = 5;

/// The built-in Macros plugin (macros and the auto-clicker), by ID in
/// [`Preferences::plugins`].
pub const MACROS_PLUGIN: &str = "pious.macros";

impl Preferences {
    pub fn migrate_defaults(&mut self) {
        if self.defaults_version < 2 {
            let look = &mut self.appearance;
            if !look.see_through && look.blur == Blur::Off {
                look.see_through = true;
                look.blur = Blur::Frosted;
            }
            if self.close_action == CloseAction::Quit {
                self.close_action = CloseAction::Tray;
            }
            if self.anti_afk.action == AfkAction::Jump {
                self.anti_afk.action = AfkAction::Walk;
            }
            // The old "Playing Roblox" default (read back as Pious).
            if self.discord_display == DiscordDisplay::Pious {
                self.discord_display = DiscordDisplay::GameName;
            }
            if self.overlay.hotkey == "Alt+Backquote" {
                self.overlay.hotkey = "Home".into();
            }
            if self.accounts_list {
                self.views.insert("accounts".into(), ViewMode::List);
            }
        }
        if self.defaults_version < 3 {
            self.views.entry("versions".into()).or_insert(ViewMode::List);
            if self.logo_action.is_empty() {
                self.logo_action = "home".into();
            }
        }
        if self.defaults_version < 4 {
            // Macros became a plugin that starts off; anyone already using
            // them keeps them on.
            if self.macros.as_array().is_some_and(|m| !m.is_empty()) || self.autoclicker["enabled"] == true {
                self.plugins.insert(MACROS_PLUGIN.into());
            }
        }
        if self.defaults_version < 5 {
            // Tweaks became on by default with every tweak neutral. Someone
            // who never changed a tweak gets the switch on (changing
            // nothing); anyone who set tweaks up and turned them off keeps
            // them off.
            if !self.tweaks.enabled && self.tweaks.is_neutral() {
                self.tweaks.enabled = true;
            }
            if self.overlay.blur_strength == 0.5 {
                self.overlay.blur_strength = 0.25;
            }
            if self.region.is_empty() {
                self.region = "auto".into();
            }
            // MOV became the default container.
            if self.recorder.container == Container::Mp4 {
                self.recorder.container = Container::Mov;
            }
        }
        self.defaults_version = DEFAULTS_VERSION;
    }
}

/// A single entry in the user's activity history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity {
    pub at: DateTime<Utc>,
    pub game_id: Uuid,
    pub account_id: Option<Uuid>,
    #[serde(default)]
    pub server: ServerChoice,
    #[serde(default)]
    pub version: Option<String>,
}

/// Extracts a place ID from a raw ID or any roblox.com game URL.
pub fn parse_place_id(input: &str) -> Option<u64> {
    let input = input.trim();
    if let Ok(id) = input.parse::<u64>() {
        return Some(id);
    }

    let url = url::Url::parse(input)
        .or_else(|_| url::Url::parse(&format!("https://{input}")))
        .ok()?;

    if let Some(id) = url
        .query_pairs()
        .find(|(k, _)| k.eq_ignore_ascii_case("placeId"))
        .and_then(|(_, v)| v.parse().ok())
    {
        return Some(id);
    }

    let mut segments = url.path_segments()?;
    while let Some(segment) = segments.next() {
        if segment.eq_ignore_ascii_case("games") {
            return segments.next()?.parse().ok();
        }
    }
    None
}

/// What a private server link points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerLink {
    /// Classic `?privateServerLinkCode=` link, with the place ID when present.
    LinkCode { place_id: Option<u64>, code: String },
    /// Modern `roblox.com/share?code=...&type=Server` link that must be resolved.
    Share { code: String },
}

pub fn parse_server_link(input: &str) -> Option<ServerLink> {
    let url = url::Url::parse(input.trim()).ok()?;
    let query = |key: &str| {
        url.query_pairs()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.into_owned())
    };

    if let Some(code) = query("privateServerLinkCode") {
        return Some(ServerLink::LinkCode {
            place_id: parse_place_id(input),
            code,
        });
    }

    if url.path().trim_end_matches('/').ends_with("/share") {
        if let Some(code) = query("code") {
            return Some(ServerLink::Share { code });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keybinds_layer_global_account_game() {
        let account = Uuid::new_v4();
        let set = |scope, binds: &[(&str, &str)]| KeybindSet {
            id: Uuid::new_v4(),
            scope,
            enabled: true,
            binds: binds.iter().map(|(f, t)| Keybind { from: (*f).into(), to: (*t).into() }).collect(),
        };
        let keybinds = Keybinds {
            enabled: true,
            sets: vec![
                set(KeybindScope::Game(42), &[("KeyQ", "KeyE")]),
                set(KeybindScope::Global, &[("KeyQ", "KeyR"), ("MouseBack", "KeyF")]),
                set(KeybindScope::Account(account), &[("MouseBack", "")]),
            ],
        };
        let mut all = keybinds.resolve(Some(account), Some(42));
        all.sort();
        assert_eq!(all, vec![("KeyQ".into(), "KeyE".into()), ("MouseBack".into(), String::new())]);
        let mut global = keybinds.resolve(None, None);
        global.sort();
        assert_eq!(global, vec![("KeyQ".into(), "KeyR".into()), ("MouseBack".into(), "KeyF".into())]);
    }

    #[test]
    fn parses_place_ids() {
        assert_eq!(parse_place_id("606849621"), Some(606849621));
        assert_eq!(
            parse_place_id("https://www.roblox.com/games/606849621/Jailbreak"),
            Some(606849621)
        );
        assert_eq!(parse_place_id("roblox.com/games/123/abc"), Some(123));
        assert_eq!(
            parse_place_id("https://www.roblox.com/games/start?placeId=42"),
            Some(42)
        );
        assert_eq!(parse_place_id("hello"), None);
    }

    #[test]
    fn parses_server_links() {
        assert_eq!(
            parse_server_link(
                "https://www.roblox.com/games/123/Name?privateServerLinkCode=abc123"
            ),
            Some(ServerLink::LinkCode {
                place_id: Some(123),
                code: "abc123".into()
            })
        );
        assert_eq!(
            parse_server_link("https://www.roblox.com/share?code=xyz&type=Server"),
            Some(ServerLink::Share { code: "xyz".into() })
        );
        assert_eq!(parse_server_link("https://example.com"), None);
    }
}
