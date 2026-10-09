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

/// Roblox experiences matching a name (Add Game's search).
#[tauri::command]
pub async fn search_games(query: String) -> Reply<Vec<crate::core::roblox::FoundGame>> {
    if query.trim().chars().count() < 2 {
        return Ok(Vec::new());
    }
    crate::core::roblox::search_games(&query).await
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
pub async fn pin_current_version(service: Service<'_>) -> Reply<String> {
    service.pin_current_version().await
}

#[tauri::command]
pub async fn pin_version(service: Service<'_>, hash: String) -> Reply {
    service.pin_version(hash).await
}

#[tauri::command]
pub fn unpin_version(service: Service<'_>) {
    service.unpin_version()
}

#[tauri::command]
pub fn set_version_profile(service: Service<'_>, name: String, guid: String) -> Reply {
    service.set_version_profile(name, guid)
}

#[tauri::command]
pub fn remove_version_profile(service: Service<'_>, name: String) {
    service.remove_version_profile(name)
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

/// The mod maker's parts: (ID, name).
#[tauri::command]
pub fn mod_maker_parts() -> Vec<(String, String)> {
    crate::core::modmaker::PARTS.iter().map(|(id, name, _)| ((*id).to_owned(), (*name).to_owned())).collect()
}

/// A few of Roblox's own pictures recolored with `ui`, from the newest
/// Roblox version installed: (part name, data: URL).
#[tauri::command]
pub async fn mod_maker_preview(service: Service<'_>, ui: crate::core::modmaker::UiMod) -> Reply<Vec<(String, String)>> {
    let build = {
        let s = service.read();
        s.bootstrapper
            .versions
            .iter()
            .filter(|v| v.path.join("RobloxPlayerBeta.exe").is_file())
            .max_by_key(|v| v.installed_at)
            .map(|v| v.path.clone())
    };
    let build = build.ok_or("Install a Roblox version first (Versions) to see a preview.")?;
    tauri::async_runtime::spawn_blocking(move || crate::core::modmaker::preview(&build, &ui, 3)).await.map_err(|e| e.to_string())
}

/// The sky presets: ID, name and a picture (data: URL).
#[tauri::command]
pub async fn skybox_previews() -> Vec<(String, String, String)> {
    tauri::async_runtime::spawn_blocking(|| {
        use base64::Engine;
        crate::core::skybox::PRESETS
            .iter()
            .filter_map(|(id, name)| {
                let png = crate::core::skybox::preview_png(id)?;
                Some(((*id).to_owned(), (*name).to_owned(), format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))))
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// The graphics cards Windows can run Roblox on, in Windows' order (for
/// "specific GPU 1, 2…").
#[tauri::command]
pub fn graphics_adapters() -> Vec<String> {
    crate::core::gpupref::adapters().into_iter().map(|a| a.name).collect()
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

/// Every cursor style: (ID, name, a picture of its pointer). Downloaded
/// once, all at the same time.
#[tauri::command]
pub async fn cursor_previews() -> Vec<(crate::core::model::CursorStyle, String, Option<String>)> {
    use base64::Engine;
    let cache = crate::core::store::data_dir().join("cache").join("mods");
    let pictures = futures::future::join_all(crate::core::tweaks::CURSORS.iter().map(|(style, _)| {
        let cache = cache.clone();
        async move {
            let (base, folder, _) = crate::core::tweaks::cursor_source(*style)?;
            let bytes = crate::core::fonts::cached_download(&format!("{base}/{folder}/ArrowCursor.png"), &cache, &format!("{folder}-ArrowCursor.png")).await.ok()?;
            Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
        }
    }))
    .await;
    crate::core::tweaks::CURSORS.iter().zip(pictures).map(|((style, name), picture)| (*style, (*name).to_owned(), picture)).collect()
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

/// Picks the folder with a custom sky's six pictures and checks them first,
/// so a folder Roblox can't use never gets saved.
#[tauri::command]
pub async fn pick_skybox_folder() -> Result<Option<String>, String> {
    let Some(folder) = rfd::AsyncFileDialog::new().set_title("The folder with your sky's six pictures").pick_folder().await else {
        return Ok(None);
    };
    let path = folder.path().to_path_buf();
    let cache = crate::core::store::data_dir().join("cache");
    let check = path.clone();
    tauri::async_runtime::spawn_blocking(move || crate::core::skybox::files("custom", Some(&check), &cache).map(|_| ()))
        .await
        .map_err(|e| e.to_string())??;
    Ok(Some(path.display().to_string()))
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

/// Shows sample pop-ups: a message and a friend starting a game. Async, so
/// it never runs on the window's thread (making the pop-up window from
/// there could hang Pious).
#[tauri::command]
pub async fn notify_test(service: Service<'_>, kind: Option<String>) -> Reply {
    use crate::service::notify::Notice;
    let id = uuid::Uuid::new_v4();
    let mut notices = Vec::new();
    if kind.as_deref() != Some("game") {
        notices.push(Notice::new(
            format!("test:{id}:message"),
            "message",
            "Pious".into(),
            "This is how a message from a friend pops up. Click it to reply.".into(),
            None,
            serde_json::json!({ "main": true }),
        ));
    }
    if kind.as_deref() != Some("message") {
        let mut game = Notice::new(
            format!("test:{id}:game"),
            "friend_join",
            "A friend".into(),
            "Started playing".into(),
            None,
            serde_json::json!({ "main": true }),
        );
        // A real game's banner, when Pious can reach Roblox.
        let place = service.read().bootstrapper.games.iter().max_by_key(|g| g.last_played).map(|g| g.place_id).unwrap_or(1_818);
        game.game = tokio::time::timeout(std::time::Duration::from_secs(4), crate::core::roblox::game_card(place)).await.ok().and_then(Result::ok);
        if let Some(card) = &game.game {
            game.body = format!("Started playing {}", card.name);
        }
        notices.push(game);
    }
    service.notify(notices);
    Ok(())
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

/// The stats overlay measured itself.
#[tauri::command]
pub async fn stats_overlay_resize(service: Service<'_>, width: f64, height: f64) -> Reply {
    service.stats_overlay_resize(width, height);
    Ok(())
}

/// Starts or finishes dragging the stats overlay into place.
#[tauri::command]
pub async fn stats_overlay_edit(service: Service<'_>, on: bool) -> Reply {
    service.stats_overlay_edit(on);
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
    // Kept small: the newest megabyte, and the one before.
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 1024 * 1024) {
        let _ = std::fs::rename(&path, path.with_extension("old.log"));
    }
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

// ── Tweaks, themes, plugins ──────────────────────────────────────────────

/// Every tweak back to normal, and what they changed undone.
#[tauri::command]
pub async fn reset_tweaks(service: Service<'_>) -> Reply {
    service.reset_tweaks().await;
    Ok(())
}

#[tauri::command]
pub async fn apply_theme(service: Service<'_>, id: String) -> Reply {
    service.apply_theme(&id)
}

/// The stylesheets, icons and sounds of enabled plugins and the theme in
/// use (also sent as `ui-assets` whenever they change).
#[tauri::command]
pub fn ui_assets(service: Service<'_>) -> crate::service::plugins::UiAssets {
    service.read().ui_assets.clone()
}

#[tauri::command]
pub async fn open_themes_folder() -> Reply {
    let dir = crate::core::store::themes_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    open::that(&dir).map_err(|e| e.to_string())
}

/// Font families installed on this PC.
#[tauri::command]
pub async fn system_fonts() -> Vec<String> {
    tokio::task::spawn_blocking(crate::core::sysfonts::installed).await.unwrap_or_default()
}

// ── Windows ──────────────────────────────────────────────────────────────

/// Lays every Roblox window out over the screens.
#[tauri::command]
pub async fn arrange_windows(service: Service<'_>, layout: Option<String>) -> Reply<usize> {
    service.arrange_windows(layout).await
}

// ── Crash reports ────────────────────────────────────────────────────────

/// A crash report's text (only files in the crash reports folder).
#[tauri::command]
pub async fn read_crash(file: std::path::PathBuf) -> Reply<String> {
    let dir = crate::core::crash::dir();
    let inside = file.canonicalize().ok().zip(dir.canonicalize().ok()).is_some_and(|(f, d)| f.starts_with(d));
    if !inside {
        return Err("That isn't a crash report.".into());
    }
    std::fs::read_to_string(file).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_crashes(service: Service<'_>) -> Reply {
    let dir = crate::core::crash::dir();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "txt")) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    service.mutate(|s| s.crashes.clear());
    Ok(())
}

#[tauri::command]
pub async fn open_crashes_folder() -> Reply {
    let dir = crate::core::crash::dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    open::that(&dir).map_err(|e| e.to_string())
}

/// The overlay's last "show", while it's open (for a window whose page
/// loaded after it was opened).
#[tauri::command]
pub fn overlay_pending() -> Option<Value> {
    crate::service::overlay::pending_show()
}

// ── Plugin API ───────────────────────────────────────────────────────────

/// A call from a plugin's page or engine, passed on by its host window
/// (which knows which plugin the frame is). See docs/PLUGIN-API.md.
#[tauri::command]
pub async fn plugin_call(service: Service<'_>, plugin: String, side: String, method: String, args: Vec<Value>) -> Reply<Value> {
    service.plugin_call(&plugin, &side, &method, args).await
}

/// The engines that should run now: each enabled plugin with a `main`.
#[tauri::command]
pub fn plugin_engines(service: Service<'_>) -> Vec<Value> {
    let s = service.read();
    crate::service::plugins::with_enabled(&s.plugins, &s.bootstrapper.preferences.plugins)
        .into_iter()
        .filter(|p| p.enabled)
        .filter_map(|p| {
            let page = crate::service::pluginapi::engine_page(&p)?;
            Some(serde_json::json!({ "id": p.id, "name": p.name, "permissions": p.permissions, "page": page }))
        })
        .collect()
}

/// Asks the plugin providing a feature for something (e.g. the overlay's
/// macro buttons: `plugin_request("macros", "run", { id })`).
#[tauri::command]
pub async fn plugin_request(service: Service<'_>, feature: String, method: String, args: Value) -> Reply<Value> {
    service.plugin_request(&feature, &method, args).await
}

/// The page `pious.exe --page <name>` asked to open (once), e.g. `games`,
/// `settings` or `plugin:<id>`.
#[tauri::command]
pub fn startup_page() -> Option<String> {
    static TAKEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if TAKEN.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == "--page").and_then(|i| args.get(i + 1)).cloned()
}

/// Roblox's regions people can pick for public servers.
#[tauri::command]
pub fn regions() -> Vec<(&'static str, &'static str)> {
    crate::core::region::REGIONS.to_vec()
}
