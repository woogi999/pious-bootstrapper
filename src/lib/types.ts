// Shapes of the data the Rust backend sends (see `service::snapshot`).
// Field names match the Rust structs, so they're snake_case.

export type Uuid = string;
export type Timestamp = string;

export type VersionChoice = "Default" | "Latest" | { Specific: string };
export type ServerChoice = "Public" | { Private: Uuid };

export interface LaunchConfig {
  account: Uuid | null;
  pin_account: boolean;
  version: VersionChoice;
  server: ServerChoice;
}

export interface Game {
  id: Uuid;
  place_id: number;
  universe_id: number | null;
  name: string;
  developer: string | null;
  description: string | null;
  favorite: boolean;
  added_at: Timestamp;
  last_played: Timestamp | null;
  play_count: number;
  config: LaunchConfig;
  collection: string | null;
  in_library: boolean;
}

export interface PrivateServer {
  id: Uuid;
  game_id: Uuid;
  name: string;
  link: string;
  server_id: string | null;
  notes: string;
  favorite: boolean;
  added_at: Timestamp;
  last_joined: Timestamp | null;
}

export interface Account {
  id: Uuid;
  user_id: number;
  username: string;
  display_name: string;
  alias: string | null;
  added_at: Timestamp;
  last_used: Timestamp | null;
  needs_sign_in: boolean;
  custom_settings: boolean;
}

export type VersionSource = "Roblox" | "Bootstrapper" | "Pious";

export interface VersionRecord {
  hash: string;
  version: string | null;
  path: string;
  source: VersionSource;
  installed_at: Timestamp;
  valid: boolean;
  label: string | null;
}

export interface Activity {
  at: Timestamp;
  game_id: Uuid;
  account_id: Uuid | null;
  server: ServerChoice;
  version: string | null;
}

export type Blur = "Off" | "Frosted" | "Diffused" | "Adjustable";

export interface Appearance {
  accent: string;
  background: string;
  surface: string;
  text: string;
  background_image: string | null;
  image_dim: number;
  image_blur: number;
  see_through: boolean;
  window_opacity: number;
  glass: number;
  blur: Blur;
  blur_strength: number;
}

export interface Preferences {
  default_account: Uuid | null;
  default_version: VersionChoice;
  multi_instance: boolean;
  remember_last_used: boolean;
  confirm_close: boolean;
  versions_dir: string | null;
  pinned: boolean;
  sidebar_collapsed: boolean;
  reduce_motion: boolean;
  auto_update: boolean;
  accounts_list: boolean;
  discord_presence: boolean;
  handle_roblox_links: boolean;
  previous_handlers: [string, string][];
  launch_via: string | null;
  tweaks: Tweaks;
  overlay: OverlayPrefs;
  anti_afk: { enabled: boolean; minutes: number; action: "Jump" | "Walk" | "Zoom" };
  auto_rejoin: boolean;
  close_action: "Quit" | "Tray";
  on_game_launch: "Stay" | "Minimize" | "HideUntilClosed";
  start_with_windows: boolean;
  start_hidden: boolean;
  ui_scale: number;
  search_blur: number;
  discord_display: "GameName" | "Pious";
  appearance: Appearance;
  views: Record<string, ViewMode>;
  recorder: RecorderPrefs;
  streamer_mode: boolean;
  link_target: "Pious" | "Browser";
  logo_action: string;
  shortcuts: Record<string, string>;
  sorts: Record<string, string>;
  onboarded: boolean;
  seen_tips: string[];
  seen_version: string;
  macros: Macro[];
  macro_settings: MacroSettings;
  autoclicker: Autoclicker;
  plugins: string[];
  mcp: { enabled: boolean; port: number; allow_actions: boolean; allow_senses: boolean };
  discord_join: boolean;
  pious_presence: boolean;
  discord_account: boolean;
  notifications: {
    enabled: boolean;
    messages: boolean;
    friend_requests: boolean;
    friend_joins: boolean;
    roblox: boolean;
    in_game: boolean;
    sound: boolean;
    seconds: number;
  };
  appear_online: boolean;
  studio_presence: boolean;
  streamer_auto: boolean;
  streamer_apps: string[];
  news_on_home: boolean;
  friends_all: boolean;
  dns: string;
  dns_custom: string[];
  server_location: boolean;
  input_overlay: InputOverlay;
  keybinds: Keybinds;
  emoji_shortcodes: { in_app: boolean; in_roblox: boolean };
}

export interface InputOverlay {
  enabled: boolean;
  layout: "streamer" | "wasd" | "fps" | "keyboard" | "mouse" | "none" | "custom";
  style: "slanted" | "flat";
  slant: number;
  rows: string[][];
  show_mouse: boolean;
  show_scroll: boolean;
  show_rates: boolean;
  show_fps: boolean;
  x: number;
  y: number;
  scale: number;
  key_size: number;
  gap: number;
  radius: number;
  border: number;
  background: string;
  key_color: string;
  pressed_color: string;
  text_color: string;
  pressed_text_color: string;
  border_color: string;
  font: string;
  font_size: number;
  bold: boolean;
  uppercase: boolean;
  animation: "none" | "pop" | "glow" | "fade";
  release_ms: number;
  opacity: number;
  only_in_game: boolean;
  only_while_capturing: boolean;
  show_in_recordings: boolean;
}

export type KeybindScope = { kind: "Global" } | { kind: "Account"; id: Uuid } | { kind: "Game"; id: number };

export interface KeybindSet {
  id: Uuid;
  scope: KeybindScope;
  enabled: boolean;
  binds: { from: string; to: string }[];
}

export interface Keybinds {
  enabled: boolean;
  sets: KeybindSet[];
}

/** What the input overlay draws (from the keyboard hook). */
export interface InputState {
  keys: string[];
  key_presses: number;
  clicks: number;
  wheel_up: number;
  wheel_down: number;
}

export interface NewsItem {
  id: string;
  kind: "updates" | "roblox" | "hyperion";
  source: string;
  title: string;
  url: string;
  date: string | null;
  summary: string | null;
}

export type InputTarget = "Roblox" | "AllRoblox" | "Accounts" | "System";

export type Press = "Tap" | "Down" | "Up";
export type MouseButton = "Left" | "Right" | "Middle" | "Back" | "Forward";

export type Step =
  | { kind: "Key"; key: string; press: Press; hold_ms: number }
  | { kind: "Text"; text: string; delay_ms: number }
  | { kind: "Click"; button: MouseButton; press: Press; count: number }
  | { kind: "Move"; x: number; y: number; relative: boolean; duration_ms: number }
  | { kind: "Scroll"; amount: number; horizontal: boolean }
  | { kind: "Wait"; ms: number; random_ms: number }
  | { kind: "Loop"; times: number; steps: Step[] }
  | { kind: "WaitPixel"; x: number; y: number; color: string; tolerance: number; timeout_ms: number }
  | { kind: "FocusRoblox" }
  | { kind: "Run"; target: string }
  | { kind: "Comment"; text: string };

export interface Macro {
  id: Uuid;
  name: string;
  hotkey: string;
  repeat: "Once" | "Times" | "UntilStopped" | "WhileHeld";
  times: number;
  speed: number;
  game_only: boolean;
  target: InputTarget;
  accounts: Uuid[];
  steps: Step[];
}

export interface MacroSettings {
  record_hotkey: string;
  stop_hotkey: string;
  record_moves: boolean;
  record_timing: boolean;
}

export interface Autoclicker {
  enabled: boolean;
  hotkey: string;
  mode: "Toggle" | "Hold" | "MouseHeld" | "OnClick";
  input: "Mouse" | "Key";
  button: MouseButton;
  key: string;
  double: boolean;
  interval_ms: number;
  random_ms: number;
  hold_ms: number;
  fixed: [number, number] | null;
  limit: "Unlimited" | "Clicks" | "Seconds";
  limit_value: number;
  game_only: boolean;
  target: InputTarget;
  accounts: Uuid[];
  trigger: MouseButton;
  burst: number;
}

export interface AutomationStatus {
  running: Uuid[];
  clicking: boolean;
  recording: boolean;
}

export interface BrowserSession {
  browser: string;
  user_id: number;
  username: string;
  display_name: string;
  added: boolean;
}

export interface Plugin {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  page: string | null;
  icon: string;
  permissions: string[];
  enabled: boolean;
  folder: string;
  problem: string | null;
  builtin: boolean;
}

export interface StatsSummary {
  play_seconds: number;
  sessions: number;
  launches: number;
  games: number;
  clips: number;
  recordings: number;
  messages: number;
  macro_runs: number;
  clicks: number;
  server_hops: number;
  app_opens: number;
  since: string | null;
  top_games: [string, number][];
}

export type ViewMode = "Grid" | "List";

export interface RecorderPrefs {
  recording: boolean;
  record_hotkey: string;
  clips: boolean;
  clip_hotkey: string;
  clip_seconds: number;
  manual_clip_hotkey: string;
  buffer_seconds: number;
  game_only: boolean;
  folder: string | null;
  file_name: string;
  container: "Mp4" | "Mkv" | "Mov";
  target: "GameWindow" | "GameMonitor";
  encoder: string;
  codec: "H264" | "Hevc" | "Av1";
  rate_control: "Quality" | "Vbr" | "Cbr";
  quality: number;
  bitrate_kbps: number;
  max_bitrate_kbps: number;
  fps: number;
  height: number | null;
  keyframe_seconds: number;
  speed: "Fastest" | "Fast" | "Balanced" | "Quality" | "Best";
  ten_bit: boolean;
  cursor: boolean;
  game_audio: "GameOnly" | "Everything" | "Off";
  game_volume: number;
  mic: boolean;
  mic_device: string | null;
  mic_volume: number;
  separate_tracks: boolean;
  audio_codec: "Aac" | "Opus";
  audio_bitrate_kbps: number;
  notify: boolean;
}

export interface RecorderStatus {
  installed: boolean;
  installing: [number, number | null] | null;
  capturing: boolean;
  recording_since: number | null;
  buffered: number;
  encoder: string | null;
  encoders: string[] | null;
  error: string | null;
  last_saved: string | null;
  folder: string;
}

export type FriendStatus = "in_game" | "in_studio" | "online" | "offline";

export interface Friend {
  id: number;
  username: string;
  display_name: string;
  avatar: string | null;
  status: FriendStatus;
  location: string | null;
  place_id: number | null;
  universe_id: number | null;
  job: string | null;
  last_online: string | null;
  /** Which of your accounts they're friends with (every-account list). */
  via: Uuid[];
}

export interface Friends {
  account: Uuid | null;
  /** The list holds the friends of every account. */
  all: boolean;
  list: Friend[];
  loading: boolean;
  error: string | null;
}

export interface Conversation {
  id: string;
  kind: string;
  name: string;
  participants: number[];
  unread: boolean;
  updated: string | null;
  preview: string | null;
}

export interface ChatMessage {
  id: string;
  sender: number;
  content: string;
  sent: string | null;
}

export interface FontPreset {
  id: string;
  label: string;
  group: string;
}

export interface ServerLinkInfo {
  place_id: number | null;
  name: string | null;
  game: string | null;
}

export interface Widget {
  x: number;
  y: number;
  pinned: boolean;
  hidden: boolean;
  /** Size in pixels once resized. */
  width?: number | null;
  height?: number | null;
}

export interface OverlayPrefs {
  enabled: boolean;
  hotkey: string;
  blur: "Off" | "Live" | "Adjustable";
  blur_strength: number;
  dim: number;
  widgets: Record<string, Widget>;
  game_only: boolean;
  media: boolean;
}

export interface NowPlaying {
  app: string;
  title: string;
  artist: string;
  playing: boolean;
  can_previous: boolean;
  can_next: boolean;
  can_pause: boolean;
  position: number | null;
  duration: number | null;
  artwork: string | null;
}

export interface FlagProfile {
  id: Uuid;
  name: string;
  flags: Record<string, string>;
}

export interface PublicServer {
  job: string;
  playing: number;
  max_players: number;
  ping: number | null;
  fps: number | null;
}

export type SettingKind = "Float" | "Int" | "Token" | "Bool";

export type GraphicsApi = "Automatic" | "Direct3D11" | "Vulkan" | "OpenGL";

export interface Tweaks {
  enabled: boolean;
  fps_limit: number | null;
  graphics: GraphicsApi;
  msaa: number;
  texture_quality: number | null;
  flag_profiles: FlagProfile[];
  active_profile: Uuid | null;
  font: string | null;
  font_preset: string | null;
  use_mods_folder: boolean;
  disable_dpi_scaling: boolean;
  alt_enter_fullscreen: boolean;
  graphics_quality: number | null;
  pause_voxelizer: boolean;
  gray_sky: boolean;
  still_grass: boolean;
  mesh_detail: number | null;
  cursor: "Default" | "From2006" | "From2013";
  old_character_sounds: boolean;
  old_avatar_background: boolean;
  emoji: EmojiStyle;
  window_title: string;
  shiftlock: string | null;
  shiftlock_color: string | null;
  shiftlock_file: string | null;
  player_icon: string | null;
  player_icon_file: string | null;
  cleaner_days: number | null;
  disable_fullscreen_optimizations: boolean;
  dpi_override: string | null;
  priority: string | null;
}

export type EmojiStyle =
  | "Default"
  | "Apple"
  | "Windows11Fluent"
  | "Windows11"
  | "Windows11Original"
  | "Windows10"
  | "Windows10Anniversary"
  | "Windows8"
  | "TwemojiSvg"
  | "EmojiOne"
  | "Catmoji"
  | "NotoMono"
  | "OpenMojiMono"
  | "Mona12"
  | "Docomo";

export interface Bootstrapper {
  games: Game[];
  servers: PrivateServer[];
  accounts: Account[];
  versions: VersionRecord[];
  activity: Activity[];
  preferences: Preferences;
}

export type InstanceStatus = "launching" | "running" | "untracked";

export interface Instance {
  id: Uuid;
  pid: number | null;
  account: Uuid | null;
  game: Uuid;
  server: ServerChoice;
  job: string | null;
  version: string | null;
  started: Timestamp;
  status: InstanceStatus;
  location: string | null;
}

export interface Install {
  version: string | null;
  downloaded: number;
  total: number;
  stage: string;
  updating: boolean;
}

export interface ClientVersion {
  version: string;
  hash: string;
}

export interface Recommendation {
  universe_id: number;
  place_id: number;
  name: string;
  creator: string;
  players: number;
  rating: number | null;
  score: number;
  because: string[];
  accounts: string[];
}

export interface InstalledBootstrapper {
  name: string;
  folder: string;
  exe: string;
  opens_links: boolean;
  discord_presence: boolean;
  activity_tracking: boolean;
  multi_instance: boolean;
  fast_flags: number;
  mods: number;
  version: string | null;
  uninstallable: boolean;
}

export interface KnownBuild {
  kind: "Current" | "Previous" | "Upcoming" | "History";
  version: string;
  hash: string;
  date: string;
}

export interface Release {
  version: string;
  notes: string;
  page: string;
}

export type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "up_to_date" }
  | { kind: "available"; release: Release }
  | { kind: "downloading"; release: Release; received: number; total: number }
  | { kind: "ready"; release: Release }
  | { kind: "failed"; error: string };

export interface Snapshot {
  loaded: boolean;
  load_error: string | null;
  bootstrapper: Bootstrapper;
  active_account: Uuid | null;
  effective: Record<Uuid, LaunchConfig>;
  usable_versions: string[];
  version_titles: Record<string, string>;
  default_version: string | null;
  latest_installed: string | null;
  instances: Instance[];
  installs: Record<string, Install>;
  latest: ClientVersion | null;
  latest_error: string | null;
  scanning: boolean;
  recommendations: Recommendation[];
  recs_loading: boolean;
  recs_error: string | null;
  bootstrappers: InstalledBootstrapper[];
  link_handler: string | null;
  builds: KnownBuild[];
  builds_loading: boolean;
  builds_error: string | null;
  update: UpdateState;
  current_version: string;
  images: Record<string, string>;
  hopping: string[];
  data_dir: string;
  versions_dir: string;
  sign_in: { verifying: boolean; reauth: Uuid | null } | null;
  tweaked_versions: string[];
  fast_flags: Record<string, unknown> | null;
  /** FastFlags Roblox refused at the last launch (from its log). */
  denied_flags: string[];
  mods_dir: string;
  recorder: RecorderStatus;
  friends: Friends;
  font_presets: FontPreset[];
  roblox_uninstallable: boolean;
  automation: AutomationStatus;
  browser_sessions: BrowserSession[];
  mcp: { url: string | null; error: string | null };
  plugins: Plugin[];
  installed: boolean;
  news: { items: NewsItem[]; loading: boolean; errors: string[] };
}

export interface GameInfo {
  place_id: number;
  universe_id: number;
  name: string;
  developer: string | null;
  description: string | null;
}

export interface UserInfo {
  id: number;
  username: string;
  display_name: string;
}

export type Presence =
  | { kind: "Offline" }
  | { kind: "Online" }
  | { kind: "InStudio" }
  | { kind: "InGame"; place_id: number | null; job: string | null; location: string };

export interface PlayerFound {
  user: UserInfo;
  presence: Presence;
  game: GameInfo | null;
}

export type QuickLoginStatus =
  | { kind: "Pending" }
  | { kind: "Linked"; account: string | null }
  | { kind: "Validated" }
  | { kind: "Cancelled" }
  | { kind: "Expired" };

export type QuickLoginUpdate =
  | { kind: "waiting"; status: QuickLoginStatus }
  | { kind: "done" }
  | { kind: "expired"; status: QuickLoginStatus };

export type LaunchOutcome =
  | { kind: "started" }
  | { kind: "stopped" }
  | { kind: "confirm"; title: string; body: string; confirm: string }
  | { kind: "sign_in"; account: Uuid }
  | { kind: "add_account" };

export interface LaunchPlan {
  game: Uuid;
  account: Uuid | null;
  version: VersionChoice;
  server: ServerChoice;
  job?: string | null;
  remember?: boolean;
  pin_account?: boolean;
  force?: boolean;
  link?: string | null;
}

export type Tone = "positive" | "active" | "caution" | "negative" | "neutral";

/** A Ctrl K search result. */
export type SearchHit =
  | { kind: "game"; id: string }
  | { kind: "server"; id: string }
  | { kind: "account"; id: string }
  | { kind: "version"; hash: string }
  | { kind: "page"; page: string; name: string; icon: string; tab?: string };
