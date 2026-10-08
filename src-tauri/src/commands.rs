//! The commands the window can call. Each is a thin wrapper over the
//! [`Service`]; results the window needs come back directly, and every
//! state change also arrives as a fresh `snapshot` event.

use serde_json::Value;
use tauri::State;
use uuid::Uuid;

use crate::core::model::{ServerChoice, VersionChoice};
use crate::core::roblox::GameInfo;
use crate::service::{LaunchOutcome, Shared, Tone, snapshot};

type Service<'a> = State<'a, Shared>;
type Reply<T = ()> = Result<T, String>;

#[tauri::command]
pub fn get_snapshot(service: Service<'_>) -> Value {
    snapshot(&service.read())
}

// ── Library ──────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn look_up_game(service: Service<'_>, input: String) -> Reply<GameInfo> {
    service.look_up_game(input).await
}

#[tauri::command]
pub fn add_game(service: Service<'_>, info: GameInfo) -> Uuid {
    service.add_game(info)
}

#[tauri::command]
pub fn add_to_library(service: Service<'_>, game: Uuid) {
    service.add_to_library(game)
}

#[tauri::command]
pub fn remove_game(service: Service<'_>, game: Uuid) {
    service.remove_game(game)
}

#[tauri::command]
pub fn toggle_favorite(service: Service<'_>, game: Uuid) {
    service.toggle_favorite(game)
}

#[tauri::command]
pub fn set_collection(service: Service<'_>, game: Uuid, name: Option<String>) {
    service.set_collection(game, name)
}

#[tauri::command]
pub fn set_launch_config(service: Service<'_>, game: Uuid, account: Option<Uuid>, version: VersionChoice, server: ServerChoice) {
    service.set_launch_config(game, account, version, server)
}

#[tauri::command]
pub async fn refresh_game(service: Service<'_>, game: Uuid) -> Reply {
    service.refresh_game(game, false).await;
    Ok(())
}

#[tauri::command]
pub async fn save_server(service: Service<'_>, form: crate::service::ServerForm) -> Reply {
    service.inner().clone().save_server(form).await
}

#[tauri::command]
pub fn remove_server(service: Service<'_>, server: Uuid) {
    service.remove_server(server)
}

#[tauri::command]
pub fn toggle_server_favorite(service: Service<'_>, server: Uuid) {
    service.toggle_server_favorite(server)
}

#[tauri::command]
pub async fn refresh_recommendations(service: Service<'_>) -> Reply {
    service.refresh_recommendations().await;
    Ok(())
}

/// A recommended game as a library entry: added with `add`, otherwise only
/// remembered under Recents once played.
#[tauri::command]
pub fn recommendation_game(service: Service<'_>, universe: u64, add: bool) -> Option<Uuid> {
    service.recommendation_game(universe, add)
}

// ── Launching ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn play(service: Service<'_>, game: Uuid, force: Option<bool>) -> LaunchOutcome {
    service.play(game, None, force.unwrap_or(false))
}

#[tauri::command]
pub fn join_server(service: Service<'_>, server: Uuid, force: Option<bool>) -> LaunchOutcome {
    service.join_server(server, force.unwrap_or(false))
}

#[tauri::command]
pub fn launch(service: Service<'_>, plan: crate::service::LaunchPlan) -> LaunchOutcome {
    service.launch(plan)
}

#[tauri::command]
pub async fn server_hop(service: Service<'_>, game: Option<Uuid>, instance: Option<Uuid>, force: Option<bool>) -> Reply<LaunchOutcome> {
    Ok(service.server_hop(game, instance, force.unwrap_or(false)).await)
}

#[tauri::command]
pub async fn find_player(service: Service<'_>, username: String, account: Uuid) -> Reply<crate::service::PlayerFound> {
    service.find_player(username, account).await
}

#[tauri::command]
pub fn join_player(
    service: Service<'_>,
    place: u64,
    job: Option<String>,
    account: Option<Uuid>,
    name: Option<String>,
    force: Option<bool>,
) -> LaunchOutcome {
    service.join_player(place, job, account, name, force.unwrap_or(false))
}

#[tauri::command]
pub async fn focus_instance(service: Service<'_>, instance: Uuid) -> Reply {
    service.focus_instance(instance).await;
    Ok(())
}

/// Sends Roblox's DNS lookups to other servers (Windows asks for
/// administrator rights), and remembers the choice.
#[tauri::command]
pub async fn set_dns(service: Service<'_>, choice: String, custom: Vec<String>) -> Reply {
    let servers = crate::core::dns::servers(&choice, &custom)?;
    let list = servers.clone();
    tokio::task::spawn_blocking(move || crate::core::dns::set(&list)).await.map_err(|e| e.to_string())??;
    service.mutate(|s| {
        s.bootstrapper.preferences.dns = choice;
        s.bootstrapper.preferences.dns_custom = custom;
        s.dirty = true;
    });
    service.toast(
        Tone::Positive,
        if servers.is_empty() { "Roblox uses your network's DNS again.".to_owned() } else { format!("Roblox's lookups go to {} now.", servers.join(", ")) },
    );
    Ok(())
}

/// The DNS providers Pious knows.
#[tauri::command]
pub fn dns_providers() -> Vec<Value> {
    crate::core::dns::PROVIDERS.iter().map(|(id, name, servers)| serde_json::json!({ "id": id, "name": name, "servers": servers })).collect()
}

/// Times looking up Roblox and Roblox's reply.
#[tauri::command]
pub async fn check_connection() -> crate::core::dns::Check {
    crate::core::dns::check().await
}

/// Clears Windows' DNS cache.
#[tauri::command]
pub async fn flush_dns(service: Service<'_>) -> Reply {
    tokio::task::spawn_blocking(crate::core::dns::flush).await.map_err(|e| e.to_string())??;
    service.toast(Tone::Positive, "Windows' DNS cache is cleared.");
    Ok(())
}

/// Another account joins the server the game in front is in.
#[tauri::command]
pub async fn quick_switch(service: Service<'_>, account: Uuid) -> Reply<LaunchOutcome> {
    Ok(service.quick_switch(account).await)
}

#[tauri::command]
pub async fn close_instance(service: Service<'_>, instance: Uuid) -> Reply {
    service.close_instance(instance).await;
    Ok(())
}

#[tauri::command]
pub async fn close_all(service: Service<'_>) -> Reply {
    service.close_all().await;
    Ok(())
}

// ── Accounts ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn add_session(service: Service<'_>, token: String, reauth: Option<Uuid>) -> Reply {
    service.add_session(token, reauth).await.map(|_| ())
}

#[tauri::command]
pub async fn quick_login_start(service: Service<'_>) -> Reply<String> {
    service.quick_login_start().await
}

#[tauri::command]
pub async fn quick_login_check(service: Service<'_>, reauth: Option<Uuid>) -> Reply<crate::service::QuickLoginUpdate> {
    service.quick_login_check(reauth).await
}

#[tauri::command]
// Async: creating a window from a synchronous command deadlocks WebView2
// on Windows (the sign-in window stayed white).
pub async fn open_sign_in(service: Service<'_>, sign_up: bool, reauth: Option<Uuid>) -> Reply {
    service.open_sign_in(sign_up, reauth)
}

#[tauri::command]
pub fn close_sign_in(service: Service<'_>) {
    service.close_sign_in()
}

#[tauri::command]
pub fn remove_account(service: Service<'_>, account: Uuid) {
    service.remove_account(account)
}

#[tauri::command]
pub fn rename_account(service: Service<'_>, account: Uuid, alias: String) {
    service.rename_account(account, alias)
}

#[tauri::command]
pub fn set_default_account(service: Service<'_>, account: Option<Uuid>) {
    service.set_default_account(account)
}

#[tauri::command]
pub fn set_active_account(service: Service<'_>, account: Uuid) {
    service.set_active_account(account)
}

// ── Versions ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn scan_versions(service: Service<'_>) -> Reply {
    service.scan_versions().await;
    Ok(())
}

#[tauri::command]
pub fn install_version(service: Service<'_>, hash: String, version: Option<String>) {
    service.spawn(move |s| async move { s.install_version(hash, version).await });
}

#[tauri::command]
pub async fn install_hash(service: Service<'_>, hash: String) -> Reply {
    service.install_hash(hash).await
}

#[tauri::command]
pub async fn remove_version(service: Service<'_>, hash: String) -> Reply {
    service.remove_version(hash).await;
    Ok(())
}

#[tauri::command]
pub fn set_default_version(service: Service<'_>, choice: VersionChoice) {
    service.set_default_version(choice)
}

#[tauri::command]
pub async fn load_builds(service: Service<'_>) -> Reply {
    service.load_builds().await;
    Ok(())
}

#[tauri::command]
pub async fn refresh_bootstrappers(service: Service<'_>) -> Reply {
    service.refresh_bootstrappers().await;
    Ok(())
}

// ── Settings ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn update_preferences(service: Service<'_>, patch: Value) -> Reply {
    service.update_preferences(patch).await
}

#[tauri::command]
pub fn reset_appearance(service: Service<'_>) {
    service.reset_appearance()
}

#[tauri::command]
pub async fn pick_background(service: Service<'_>) -> Reply {
    service.pick_background().await;
    Ok(())
}

#[tauri::command]
pub async fn set_handle_links(service: Service<'_>, on: bool) -> Reply {
    service.set_handle_links(on).await;
    Ok(())
}

#[tauri::command]
pub async fn check_for_update(service: Service<'_>) -> Reply {
    service.check_for_update(true).await;
    Ok(())
}

#[tauri::command]
pub async fn download_update(service: Service<'_>) -> Reply {
    service.download_update().await;
    Ok(())
}

#[tauri::command]
pub async fn restart_to_update(service: Service<'_>) -> Reply {
    service.restart_to_update().await;
    Ok(())
}

// ── System ───────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn open_url(service: Service<'_>, url: String) -> Reply {
    service.open_web_link(&url);
    Ok(())
}

/// Opens a link in the web browser, whatever the setting.
#[tauri::command]
pub fn open_external(service: Service<'_>, url: String) {
    if !(url.starts_with("https://") || url.starts_with("http://")) || open::that_detached(&url).is_err() {
        service.toast(Tone::Negative, "Couldn't open your browser.");
    }
}

// ── Windows ──────────────────────────────────────────────────────────────

/// Async: it closes the very window that calls it.
#[tauri::command]
pub async fn splash_done(service: Service<'_>) -> Reply {
    service.splash_done();
    Ok(())
}

#[tauri::command]
pub async fn open_chat_window(service: Service<'_>, account: Option<Uuid>, user: Option<u64>) -> Reply {
    service.open_chat_window(account, user)
}

#[tauri::command]
pub fn changelog() -> &'static str {
    crate::service::CHANGELOG
}

#[tauri::command]
pub async fn stats_summary() -> crate::core::stats::Summary {
    tokio::task::spawn_blocking(crate::core::stats::summary).await.unwrap_or_default()
}

// ── Accounts from browsers ───────────────────────────────────────────────

#[tauri::command]
pub async fn find_browser_sessions(service: Service<'_>) -> Reply<crate::service::BrowserSearch> {
    service.find_browser_sessions().await
}

#[tauri::command]
pub async fn add_browser_session(service: Service<'_>, user: u64, reauth: Option<Uuid>) -> Reply {
    service.add_browser_session(user, reauth).await
}

// ── Macros and the auto-clicker ──────────────────────────────────────────

#[tauri::command]
pub fn run_macro(service: Service<'_>, id: Uuid) -> Reply {
    service.run_macro(id)
}

#[tauri::command]
pub fn stop_automation(service: Service<'_>) {
    service.stop_automation()
}

#[tauri::command]
pub fn toggle_autoclicker(service: Service<'_>) {
    service.toggle_autoclicker()
}

#[tauri::command]
pub fn toggle_macro_recording(service: Service<'_>) {
    service.toggle_macro_recording()
}

#[tauri::command]
pub fn cursor_info() -> Value {
    crate::service::automation::cursor_info()
}

// ── Chats, from the cache ────────────────────────────────────────────────

#[tauri::command]
pub fn cached_messages(service: Service<'_>, account: Uuid, conversation: String) -> Vec<crate::core::social::Message> {
    service.cached_messages(account, &conversation)
}

#[tauri::command]
pub fn cached_conversations(service: Service<'_>, account: Uuid) -> Vec<crate::core::social::Conversation> {
    service.cached_conversations(account)
}

// ── Plugins and MCP ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn create_example_plugin(service: Service<'_>) -> Reply {
    let folder = crate::service::plugins::create_example()?;
    let _ = open::that_detached(&folder);
    service.refresh_plugins().await;
    Ok(())
}

#[tauri::command]
pub async fn open_plugins_folder(service: Service<'_>) -> Reply {
    let folder = crate::service::plugins::dir();
    let _ = std::fs::create_dir_all(&folder);
    if open::that_detached(&folder).is_err() {
        service.toast(Tone::Negative, "Couldn't open that folder.");
    }
    service.refresh_plugins().await;
    Ok(())
}

/// How AI apps connect: the address, token and a ready-made config.
#[tauri::command]
pub fn mcp_info(service: Service<'_>) -> Value {
    let port = service.read().bootstrapper.preferences.mcp.port;
    let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
    serde_json::json!({
        "url": format!("http://127.0.0.1:{port}/mcp"),
        "token": crate::service::mcp::token(),
        "exe": exe,
    })
}

/// The AI apps Pious can connect itself to, and whether each already is.
#[tauri::command]
pub fn mcp_apps() -> Vec<Value> {
    crate::core::mcpsetup::APPS
        .iter()
        .map(|(id, name)| {
            serde_json::json!({
                "id": id,
                "name": name,
                "connected": crate::core::mcpsetup::is_connected(id),
                "path": crate::core::mcpsetup::config_path(id).map(|p| p.display().to_string()),
            })
        })
        .collect()
}

/// Adds Pious to an AI app's MCP settings (or takes it out).
#[tauri::command]
pub fn mcp_connect(service: Service<'_>, app: String, on: bool) -> Reply {
    let name = crate::core::mcpsetup::APPS.iter().find(|(id, _)| *id == app).map(|(_, n)| *n).unwrap_or("That app");
    if on {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        crate::core::mcpsetup::connect(&app, &exe)?;
        if !service.read().bootstrapper.preferences.mcp.enabled {
            service.mutate(|s| {
                s.bootstrapper.preferences.mcp.enabled = true;
                s.dirty = true;
            });
            service.restart_mcp();
        }
        service.toast(Tone::Positive, format!("{name} can use Pious now. Restart {name} if it's open."));
    } else {
        crate::core::mcpsetup::disconnect(&app)?;
        service.toast(Tone::Neutral, format!("Took Pious out of {name}."));
    }
    Ok(())
}

/// FastFlags saved by installed bootstrappers (Bloxstrap, Fishstrap,
/// Froststrap…), to import: name → flags.
#[tauri::command]
pub async fn bootstrapper_flags() -> Reply<Vec<(String, serde_json::Map<String, Value>)>> {
    tokio::task::spawn_blocking(|| {
        crate::core::bootstrappers::saved_flags().into_iter().map(|(name, flags)| (name.to_owned(), flags)).collect()
    })
    .await
    .map_err(|e| e.to_string())
}

/// Lets the user pick a JSON file and returns what's in it (None when
/// nothing was picked).
#[tauri::command]
pub async fn read_json_file(title: String) -> Reply<Option<String>> {
    let Some(file) = rfd::AsyncFileDialog::new().set_title(&title).add_filter("JSON", &["json", "txt"]).pick_file().await else {
        return Ok(None);
    };
    let bytes = file.read().await;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("That file is too big to be FastFlags.".into());
    }
    // Some editors save with a byte-order mark.
    let text = String::from_utf8_lossy(&bytes);
    Ok(Some(text.trim_start_matches('\u{feff}').to_owned()))
}

/// Saves text to a JSON file the user picks.
#[tauri::command]
pub async fn save_json_file(service: Service<'_>, name: String, text: String) -> Reply {
    let Some(file) = rfd::AsyncFileDialog::new().set_file_name(&name).add_filter("JSON", &["json"]).save_file().await else {
        return Ok(());
    };
    file.write(text.as_bytes()).await.map_err(|e| e.to_string())?;
    service.toast(Tone::Positive, format!("Saved {}", file.file_name()));
    Ok(())
}

/// Pictures of the shift lock cursor presets (data: URLs), in `color`.
#[tauri::command]
pub fn shiftlock_previews(color: Option<String>) -> Vec<(String, String, String)> {
    crate::core::crosshair::PRESETS
        .iter()
        .map(|(id, name)| {
            use base64::Engine;
            let png = crate::core::crosshair::render(id, color.as_deref());
            ((*id).to_owned(), (*name).to_owned(), format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)))
        })
        .collect()
}

/// The Roblox icon presets: ID, name and a picture (data: URL, downloaded
/// the first time).
#[tauri::command]
pub async fn player_icon_previews() -> Vec<(String, String, Option<String>)> {
    let cache = crate::core::store::data_dir().join("cache").join("mods");
    let mut out = Vec::new();
    for (id, name, _) in crate::core::playericon::PRESETS {
        let picture = crate::core::playericon::load(id, None, &cache).await.ok().map(|i| crate::core::playericon::preview(&i));
        out.push((id.to_owned(), name.to_owned(), picture));
    }
    out
}

/// Picks a picture file (PNG, ICO, JPG). Returns its path.
#[tauri::command]
pub async fn pick_picture(title: String) -> Option<String> {
    let file = rfd::AsyncFileDialog::new()
        .set_title(&title)
        .add_filter("Pictures", &["png", "ico", "jpg", "jpeg", "bmp"])
        .pick_file()
        .await?;
    Some(file.path().display().to_string())
}

/// Makes a macro from an AutoHotkey script: pasted `text`, or a file the
/// user picks. Returns the new macro's ID and how many lines were kept as
/// notes (not run), or nothing if the user cancelled.
#[tauri::command]
pub async fn import_ahk(service: Service<'_>, text: Option<String>) -> Reply<Option<serde_json::Value>> {
    let (text, name) = match text {
        Some(text) => (text, "AutoHotkey macro".to_owned()),
        None => {
            let Some(file) = rfd::AsyncFileDialog::new()
                .set_title("Import an AutoHotkey script")
                .add_filter("AutoHotkey", &["ahk", "ah2", "txt"])
                .pick_file()
                .await
            else {
                return Ok(None);
            };
            let bytes = file.read().await;
            if bytes.len() > 1024 * 1024 {
                return Err("That file is too big to be a macro.".into());
            }
            let name = file.file_name().rsplit_once('.').map(|(n, _)| n.to_owned()).unwrap_or_else(|| file.file_name());
            (String::from_utf8_lossy(&bytes).trim_start_matches('\u{feff}').to_owned(), name)
        }
    };
    let imported = crate::core::ahk::import(&text);
    if imported.steps.is_empty() {
        return Err("There was nothing Pious could turn into steps in that script.".into());
    }
    let m = crate::core::automation::Macro {
        name,
        hotkey: imported.hotkey.unwrap_or_default(),
        steps: imported.steps,
        ..Default::default()
    };
    let id = m.id;
    service.mutate(|s| {
        s.bootstrapper.preferences.macros.push(m);
        s.dirty = true;
    });
    service.apply_hotkeys(false);
    Ok(Some(serde_json::json!({ "id": id, "skipped": imported.skipped })))
}

/// Saves a macro as an AutoHotkey v2 script the user can run elsewhere.
#[tauri::command]
pub async fn export_ahk(service: Service<'_>, id: Uuid) -> Reply {
    let found = service.read().bootstrapper.preferences.macros.iter().find(|m| m.id == id).cloned();
    let m = found.ok_or("That macro isn't there anymore.")?;
    let script = crate::core::ahk::export(&m);
    let safe: String = m.name.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_file_name(format!("{}.ahk", safe.trim()))
        .add_filter("AutoHotkey", &["ahk"])
        .save_file()
        .await
    else {
        return Ok(());
    };
    file.write(script.as_bytes()).await.map_err(|e| e.to_string())?;
    service.toast(Tone::Positive, format!("Saved {}", file.file_name()));
    Ok(())
}

/// Puts a shortcut on the desktop that opens a game with Pious.
#[tauri::command]
pub async fn create_shortcut(service: Service<'_>, game: Uuid) -> Reply {
    let found = service.read().bootstrapper.game(game).map(|g| (g.name.clone(), g.place_id));
    let (name, place) = found.ok_or("That game isn't in Pious anymore.")?;
    let path = tokio::task::spawn_blocking(move || crate::platform::system::create_game_shortcut(&name, place))
        .await
        .map_err(|e| e.to_string())??;
    let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    service.toast(Tone::Positive, format!("Made a shortcut on your desktop: {file}"));
    Ok(())
}

/// Fetches the news again (`force`: even if it's fresh).
#[tauri::command]
pub async fn refresh_news(service: Service<'_>, force: Option<bool>) -> Reply {
    service.refresh_news(force.unwrap_or(false)).await;
    Ok(())
}

/// The notification pop-ups measured themselves (logical pixels).
#[tauri::command]
pub async fn notify_resize(service: Service<'_>, height: f64) -> Reply {
    service.notify_resize(height);
    Ok(())
}

/// Shows a made-up notification, to see how they look.
#[tauri::command]
pub fn notify_take() -> Vec<Value> {
    crate::service::notify::take_notices()
}

/// Shows a sample pop-up.
#[tauri::command]
pub fn notify_test(service: Service<'_>) {
    service.notify(vec![crate::service::notify::Notice {
        id: format!("test:{}", uuid::Uuid::new_v4()),
        kind: "message",
        title: "Pious".into(),
        body: "This is how a message from a friend pops up.".into(),
        avatar: None,
        action: serde_json::json!({ "main": true }),
    }]);
}

/// The input overlay measured itself (logical pixels).
#[tauri::command]
pub async fn input_overlay_resize(service: Service<'_>, width: f64, height: f64) -> Reply {
    service.input_overlay_resize(width, height);
    Ok(())
}

/// Starts or finishes dragging the input overlay into place.
#[tauri::command]
pub async fn input_overlay_edit(service: Service<'_>, on: bool) -> Reply {
    service.input_overlay_edit(on);
    Ok(())
}

/// What the input overlay should draw right now.
#[tauri::command]
pub fn input_state() -> crate::core::inputhook::Held {
    crate::core::inputhook::held()
}

/// What's playing on the PC, for the overlay's media controls.
#[tauri::command]
pub async fn media_now_playing() -> Option<crate::core::media::NowPlaying> {
    tokio::task::spawn_blocking(crate::core::media::now_playing).await.ok().flatten()
}

/// Presses play/pause, next or previous on what's playing.
#[tauri::command]
pub async fn media_control(action: crate::core::media::Action) -> Result<(), String> {
    tokio::task::spawn_blocking(move || crate::core::media::control(action)).await.map_err(|e| e.to_string())?
}

/// Replaces the changed in-app shortcuts (a merge can't remove one).
#[tauri::command]
pub fn set_shortcuts(service: Service<'_>, shortcuts: std::collections::BTreeMap<String, String>) {
    service.mutate(|s| {
        s.bootstrapper.preferences.shortcuts = shortcuts;
        s.dirty = true;
    });
}

// ── Fonts ────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn font_preview(service: Service<'_>, preset: String) -> Reply<std::path::PathBuf> {
    service.font_preview(preset).await
}

#[tauri::command]
pub fn open_path(service: Service<'_>, path: std::path::PathBuf) {
    let _ = std::fs::create_dir_all(&path);
    if open::that_detached(&path).is_err() {
        service.toast(Tone::Negative, "Couldn't open that folder.");
    }
}

#[tauri::command]
pub async fn set_link_handler(service: Service<'_>, target: String) -> Reply {
    service.set_link_handler(target).await;
    Ok(())
}

#[tauri::command]
pub async fn pick_font(service: Service<'_>) -> Reply {
    service.pick_font().await;
    Ok(())
}

#[tauri::command]
pub fn open_mods_folder(service: Service<'_>) {
    let dir = crate::core::tweaks::mods_dir(&crate::core::store::data_dir());
    let _ = std::fs::create_dir_all(&dir);
    if open::that_detached(&dir).is_err() {
        service.toast(Tone::Negative, "Couldn't open that folder.");
    }
}

#[tauri::command]
pub fn toggle_overlay(service: Service<'_>) {
    service.toggle_overlay()
}

#[tauri::command]
pub fn hide_overlay(service: Service<'_>) {
    service.hide_overlay()
}

#[tauri::command]
pub fn finish_hide_overlay(service: Service<'_>) {
    service.finish_hide_overlay()
}

/// The title bar's close button: quit, or keep running in the tray.
#[tauri::command]
pub async fn close_window(service: Service<'_>) -> Reply {
    use tauri::Manager;
    let to_tray = service.read().bootstrapper.preferences.close_action == crate::core::model::CloseAction::Tray;
    if to_tray {
        if let Some(window) = service.app().get_webview_window("main") {
            let _ = window.hide();
        }
        return Ok(());
    }
    service.save_now().await;
    service.app().exit(0);
    Ok(())
}

#[tauri::command]
pub async fn list_servers(service: Service<'_>, game: Uuid, refresh: Option<bool>) -> Reply<Vec<crate::core::roblox::PublicServer>> {
    service.list_servers(game, refresh.unwrap_or(false)).await
}

#[tauri::command]
pub fn join_job(service: Service<'_>, game: Uuid, job: String, force: Option<bool>) -> LaunchOutcome {
    service.join_job(game, job, force.unwrap_or(false))
}

#[tauri::command]
pub fn set_custom_settings(service: Service<'_>, account: Uuid, on: bool) {
    service.set_custom_settings(account, on)
}

/// The editable Roblox settings of an account (or the PC's, for `None`).
#[tauri::command]
pub async fn get_roblox_settings(account: Option<Uuid>) -> Reply<Vec<(String, crate::core::robloxsettings::Kind, String)>> {
    tokio::task::spawn_blocking(move || crate::core::robloxsettings::Profiles::new(&crate::core::store::data_dir()).values(account))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_roblox_settings(service: Service<'_>, account: Option<Uuid>, values: Vec<(String, String)>) -> Reply {
    let result = tokio::task::spawn_blocking(move || {
        crate::core::robloxsettings::Profiles::new(&crate::core::store::data_dir()).set_values(account, &values)
    })
    .await
    .map_err(|e| e.to_string())?;
    match result {
        Ok(()) => {
            service.toast(Tone::Positive, "Roblox settings saved. They apply the next time Roblox starts.");
            Ok(())
        }
        Err(error) => Err(error),
    }
}

/// Copies the settings Roblox uses right now into an account's own.
#[tauri::command]
pub async fn import_roblox_settings(service: Service<'_>, account: Option<Uuid>) -> Reply {
    tokio::task::spawn_blocking(move || {
        crate::core::robloxsettings::Profiles::new(&crate::core::store::data_dir()).import_live(account)
    })
    .await
    .map_err(|e| e.to_string())??;
    service.toast(Tone::Positive, "Imported the Roblox settings this PC is using now.");
    Ok(())
}

/// A private server link's game and name (for the form).
#[tauri::command]
pub async fn server_link_info(service: Service<'_>, link: String) -> Reply<crate::service::ServerLinkInfo> {
    service.server_link_info(link).await
}

/// Joins a private server from its link without saving it.
#[tauri::command]
pub async fn join_link(service: Service<'_>, link: String, account: Option<Uuid>, force: Option<bool>) -> Reply<LaunchOutcome> {
    Ok(service.inner().clone().join_link(link, account, force.unwrap_or(false)).await)
}

// ── Friends & chat ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn refresh_friends(service: Service<'_>, account: Option<Uuid>) -> Reply {
    service.inner().clone().refresh_friends(account).await;
    Ok(())
}

#[tauri::command]
pub async fn conversations(service: Service<'_>, account: Uuid) -> Reply<Vec<crate::core::social::Conversation>> {
    service.conversations(account).await
}

#[tauri::command]
pub async fn chat_messages(service: Service<'_>, account: Uuid, conversation: String) -> Reply<Vec<crate::core::social::Message>> {
    service.chat_messages(account, conversation).await
}

#[tauri::command]
pub async fn open_chat(service: Service<'_>, account: Uuid, user: u64) -> Reply<String> {
    service.open_chat(account, user).await
}

#[tauri::command]
pub async fn send_chat(service: Service<'_>, account: Uuid, conversation: String, text: String) -> Reply {
    service.send_chat(account, conversation, text).await
}

// ── Recording ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn install_recorder(service: Service<'_>) -> Reply {
    service.inner().clone().install_recorder().await;
    Ok(())
}

#[tauri::command]
pub async fn check_encoders(service: Service<'_>) -> Reply<Vec<String>> {
    Ok(service.inner().clone().check_encoders().await)
}

#[tauri::command]
pub async fn microphones() -> Reply<Vec<(String, String)>> {
    tokio::task::spawn_blocking(crate::core::recorder::microphones).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_recording(service: Service<'_>) {
    service.inner().clone().toggle_recording()
}

#[tauri::command]
pub fn save_clip(service: Service<'_>, seconds: Option<u32>) {
    service.inner().clone().save_clip(seconds, std::time::Instant::now())
}

#[tauri::command]
pub fn submit_clip(service: Service<'_>, seconds: Option<u32>) {
    service.inner().clone().submit_clip(seconds)
}

#[tauri::command]
pub fn hide_hud(service: Service<'_>) {
    service.hide_hud()
}

/// Chooses where videos are saved.
#[tauri::command]
pub async fn pick_videos_folder(service: Service<'_>) -> Reply {
    if let Some(folder) = rfd::AsyncFileDialog::new().set_title("Save videos in").pick_folder().await {
        let path = folder.path().to_path_buf();
        service.mutate(|s| {
            s.bootstrapper.preferences.recorder.folder = Some(path);
            s.dirty = true;
        });
    }
    Ok(())
}

// ── Uninstalling ─────────────────────────────────────────────────────────

/// Runs the uninstaller of a bootstrapper, or of Roblox itself ("Roblox").
#[tauri::command]
pub async fn uninstall_app(service: Service<'_>, name: String) -> Reply {
    let command = crate::platform::system::uninstall_command(&name)
        .ok_or_else(|| format!("{name} has no uninstaller registered with Windows."))?;
    crate::platform::system::run_command_line(&command)?;
    service.toast(Tone::Neutral, format!("Opened {name}'s uninstaller."));
    let me = service.inner().clone();
    me.spawn(|s| async move {
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        s.refresh_bootstrappers().await;
        s.scan_versions().await;
    });
    Ok(())
}

/// Switches the main window in and out of fullscreen.
#[tauri::command]
pub fn toggle_fullscreen(window: tauri::WebviewWindow) -> Reply<bool> {
    let full = window.is_fullscreen().map_err(|e| e.to_string())?;
    if !full && window.is_maximized().unwrap_or(false) {
        // Fullscreen doesn't take over a maximized frameless window.
        let _ = window.unmaximize();
    }
    window.set_fullscreen(!full).map_err(|e| e.to_string())?;
    Ok(!full)
}

/// Clears old Roblox logs and cache now.
#[tauri::command]
pub async fn clean_roblox(service: Service<'_>) -> Reply {
    let days = service.read().bootstrapper.preferences.tweaks.cleaner_days.unwrap_or(7);
    let removed = tokio::task::spawn_blocking(move || crate::core::cleaner::clean(days)).await.unwrap_or(0);
    service.toast(Tone::Positive, format!("Removed {removed} old Roblox file{}.", if removed == 1 { "" } else { "s" }));
    Ok(())
}

/// Brings the main window to the front (from the overlay).
#[tauri::command]
pub fn show_main(service: Service<'_>) {
    service.show_main()
}

/// Writes an error from the window to `ui-errors.log` in the data folder,
/// so problems in the interface can be diagnosed afterwards.
#[tauri::command]
pub fn report_error(message: String) {
    use std::io::Write;
    let path = crate::core::store::data_dir().join("ui-errors.log");
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{} {message}", chrono::Utc::now().to_rfc3339());
    }
}

/// Saves the library, then closes the app.
#[tauri::command]
pub async fn quit(service: Service<'_>) -> Reply {
    service.save_now().await;
    service.app().exit(0);
    Ok(())
}
