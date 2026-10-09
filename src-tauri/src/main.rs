//! Pious — your Roblox games, accounts, private
//! servers, versions and running instances.
//!
//! The interface is a Svelte app in a Tauri window (`../src`); everything
//! else, from the library to launching Roblox, is Rust (`service`, `core`).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// The window's snapshot is one big `json!`.
#![recursion_limit = "256"]

mod commands;
mod core;
mod platform;
mod service;

use tauri::{Manager, WindowEvent};

use crate::service::Service;

fn main() {
    // An AI app talking to Pious over stdio (see service::mcp).
    if std::env::args().any(|a| a == "--mcp") {
        service::mcp::stdio_bridge();
        return;
    }
    let mut first = platform::system::claim_single_instance();
    // Just updated: the old version is still closing; wait for it.
    if !first && std::env::args().any(|a| a == core::updater::AFTER_UPDATE) {
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(200));
            first = platform::system::claim_single_instance();
            if first {
                break;
            }
        }
    }

    // Opened for a Roblox link (Play on roblox.com) while Pious handles
    // them: queue it, and let the copy that's already running launch it.
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--roblox-link" {
            if let Some(link) = args.next() {
                let _ = core::store::post_link(&link);
            }
            if !first {
                return;
            }
        }
    }

    // Pious is already open: bring that copy forward instead of running
    // a second one that would overwrite its library.
    if !first {
        let _ = core::store::post_link(service::SHOW_MAIN);
        return;
    }

    core::updater::clean_up();
    // Older data folders move to where this copy keeps its data (before
    // anything reads them).
    core::store::migrate();
    core::crash::install_panic_hook();
    // Before any window or Discord connection exists.
    core::discord::register();

    tauri::Builder::default()
        .plugin(service::hotkeys::plugin())
        .setup(|app| {
            // The window shows cached artwork and the background picture
            // straight from the data folder.
            let _ = app.asset_protocol_scope().allow_directory(core::store::data_dir(), true);
            // Themes' and plugins' pictures, fonts and icons.
            // (Every folder they're read from: in development builds also the
            // repository's own, or plugin pages and engines load blank.)
            for dir in service::plugins::dirs().into_iter().chain(service::themes::dirs()) {
                let _ = app.asset_protocol_scope().allow_directory(dir, true);
            }

            let service = Service::new(app.handle().clone());
            app.manage(service.clone());
            tray(app)?;
            service::windows::remember_main_thread();
            // Started with Windows in the background: no logo either.
            if std::env::args().any(|a| a == "--background") {
                if let Some(splash) = app.get_webview_window(service::windows::SPLASH_WINDOW) {
                    let _ = splash.close();
                }
            }
            let failsafe = service.clone();
            tauri::async_runtime::spawn(async move { failsafe.splash_failsafe().await });
            tauri::async_runtime::spawn(platform::memory::watch(app.handle().clone()));
            tauri::async_runtime::spawn(async move { service.start().await });
            Ok(())
        })
        .on_window_event(|window, event| {
            // Back in front: full speed again.
            if let WindowEvent::Focused(true) = event {
                if let Some(webview) = window.app_handle().get_webview_window(window.label()) {
                    platform::memory::focused(&webview);
                }
                // Pious is in front: the taskbar badge has been seen.
                if window.label() == "main" {
                    if let Some(service) = window.app_handle().try_state::<service::Shared>() {
                        service::taskbar::seen(&service);
                    }
                }
            }
            // The overlay closes when it loses focus (clicking back into the game).
            if let (WindowEvent::Focused(false), service::overlay::OVERLAY_WINDOW) = (event, window.label()) {
                if let Some(service) = window.app_handle().try_state::<service::Shared>() {
                    if window.is_visible().unwrap_or(false) {
                        service.hide_overlay();
                    }
                }
            }
            // Save before the main window closes (or go to the tray, if
            // that's what closing does).
            if let (WindowEvent::CloseRequested { api, .. }, "main") = (event, window.label()) {
                api.prevent_close();
                let to_tray = window
                    .app_handle()
                    .try_state::<service::Shared>()
                    .is_some_and(|s| s.read().bootstrapper.preferences.close_action == core::model::CloseAction::Tray);
                if to_tray {
                    let _ = window.hide();
                    return;
                }
                let app = window.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(service) = app.try_state::<service::Shared>() {
                        service.save_now().await;
                    }
                    app.exit(0);
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::look_up_game,
            commands::add_game,
            commands::add_to_library,
            commands::remove_game,
            commands::toggle_favorite,
            commands::set_collection,
            commands::set_launch_config,
            commands::refresh_game,
            commands::save_server,
            commands::remove_server,
            commands::toggle_server_favorite,
            commands::refresh_recommendations,
            commands::recommendation_game,
            commands::play,
            commands::join_server,
            commands::launch,
            commands::server_hop,
            commands::find_player,
            commands::join_player,
            commands::quick_switch,
            commands::set_dns,
            commands::dns_providers,
            commands::check_connection,
            commands::flush_dns,
            commands::focus_instance,
            commands::close_instance,
            commands::close_all,
            commands::add_session,
            commands::quick_login_start,
            commands::quick_login_check,
            commands::open_sign_in,
            commands::close_sign_in,
            commands::remove_account,
            commands::rename_account,
            commands::set_default_account,
            commands::set_active_account,
            commands::scan_versions,
            commands::install_version,
            commands::install_hash,
            commands::remove_version,
            commands::set_default_version,
            commands::load_builds,
            commands::refresh_bootstrappers,
            commands::update_preferences,
            commands::reset_appearance,
            commands::pick_background,
            commands::set_handle_links,
            commands::check_for_update,
            commands::download_update,
            commands::restart_to_update,
            commands::open_url,
            commands::open_path,
            commands::set_link_handler,
            commands::pick_font,
            commands::open_mods_folder,
            commands::toggle_overlay,
            commands::hide_overlay,
            commands::finish_hide_overlay,
            commands::close_window,
            commands::list_servers,
            commands::join_job,
            commands::set_custom_settings,
            commands::get_roblox_settings,
            commands::set_roblox_settings,
            commands::clean_roblox,
            commands::show_main,
            commands::report_error,
            commands::import_roblox_settings,
            commands::server_link_info,
            commands::join_link,
            commands::refresh_friends,
            commands::conversations,
            commands::chat_messages,
            commands::open_chat,
            commands::send_chat,
            commands::install_recorder,
            commands::check_encoders,
            commands::microphones,
            commands::toggle_recording,
            commands::save_clip,
            commands::submit_clip,
            commands::hide_hud,
            commands::pick_videos_folder,
            commands::uninstall_app,
            commands::toggle_fullscreen,
            commands::quit,
            commands::open_external,
            commands::splash_done,
            commands::open_chat_window,
            commands::changelog,
            commands::stats_summary,
            commands::find_browser_sessions,
            commands::add_browser_session,
            commands::cached_messages,
            commands::cached_conversations,
            commands::create_example_plugin,
            commands::open_plugins_folder,
            commands::mcp_info,
            commands::font_preview,
            commands::set_shortcuts,
            commands::input_overlay_resize,
            commands::input_overlay_edit,
            commands::stats_overlay_resize,
            commands::stats_overlay_edit,
            commands::input_state,
            commands::media_now_playing,
            commands::notify_resize,
            commands::mcp_apps,
            commands::mcp_connect,
            commands::shiftlock_previews,
            commands::player_icon_previews,
            commands::pick_picture,
            commands::notify_test,
            commands::notify_take,
            commands::media_control,
            commands::refresh_news,
            commands::bootstrapper_flags,
            commands::read_json_file,
            commands::save_json_file,
            commands::create_shortcut,
            commands::reset_tweaks,
            commands::apply_theme,
            commands::ui_assets,
            commands::open_themes_folder,
            commands::system_fonts,
            commands::arrange_windows,
            commands::read_crash,
            commands::clear_crashes,
            commands::open_crashes_folder,
            commands::regions,
            commands::overlay_pending,
            commands::plugin_call,
            commands::plugin_engines,
            commands::plugin_request,
            commands::cursor_previews,
            commands::skybox_previews,
            commands::search_games,
            commands::pick_skybox_folder,
            commands::graphics_adapters,
            commands::startup_page,
        ])
        .build(tauri::generate_context!())
        .expect("error while running Pious")
        .run(|_, event| {
            // Pious lives in the tray: closing its last window (the main
            // window is closed while hidden, to save memory) isn't quitting.
            // Quit asks with an exit code, which still goes through.
            if let tauri::RunEvent::ExitRequested { code: None, api, .. } = event {
                api.prevent_exit();
            }
        });
}

/// The tray icon: click to show Pious, right-click for Open / Quit.
fn tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let open = MenuItem::with_id(app, "open", "Open Pious", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut builder = TrayIconBuilder::with_id("pious")
        .tooltip("Pious")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show(app),
            "quit" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(service) = app.try_state::<service::Shared>() {
                        service.save_now().await;
                    }
                    app.exit(0);
                });
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn show(app: &tauri::AppHandle) {
    if let Some(service) = app.try_state::<service::Shared>() {
        service.open_main(true);
    }
}
