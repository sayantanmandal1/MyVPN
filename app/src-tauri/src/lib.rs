//! MyVPN — application entry point, plugin wiring, system tray, and lifecycle.

mod commands;
mod error;
mod public_vpn;
mod state;
mod vpn;

use std::sync::Arc;

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};

use public_vpn::PublicVpn;
use vpn::VpnEngine;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    // The single-instance plugin must be registered first so a second launch
    // simply focuses the existing window instead of spawning a duplicate.
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }));
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(|app| {
            init_tracing();

            let handle = app.handle().clone();
            let key_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir());
            let _ = std::fs::create_dir_all(&key_dir);
            let engine = Arc::new(VpnEngine::new(handle.clone(), key_dir.clone()));
            app.manage(engine.clone());

            // Apply the persisted self-hosted relay (if any) before any session.
            engine.set_relay_url(commands::load_settings(&handle).relay_url);

            // The public-VPN mode is a separate, independently-managed subsystem.
            let public = Arc::new(PublicVpn::new(handle.clone(), key_dir));
            app.manage(public);

            build_tray(app)?;

            // Closing the window hides it to the tray instead of quitting, so
            // hosting keeps running in the background.
            if let Some(win) = app.get_webview_window("main") {
                let win_for_event = win.clone();
                let handle_for_event = handle.clone();
                win.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        // Honor the user's preference: hide to tray, or quit.
                        if commands::load_settings(&handle_for_event).minimize_to_tray {
                            let _ = win_for_event.hide();
                            api.prevent_close();
                        } else {
                            handle_for_event.exit(0);
                        }
                    }
                });

                // When launched at login via autostart we start hidden.
                if std::env::args().any(|a| a == "--minimized") {
                    let _ = win.hide();
                }
            }

            commands::maybe_resume_hosting(&handle, engine);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::list_discovered,
            commands::start_host,
            commands::stop_host,
            commands::connect,
            commands::disconnect,
            commands::generate_ticket,
            commands::get_settings,
            commands::save_settings,
            commands::set_autostart,
            commands::get_autostart,
            commands::show_window,
            commands::quit_app,
            commands::public_refresh,
            commands::public_servers,
            commands::public_connect,
            commands::public_disconnect,
            commands::public_status,
            commands::check_update,
            commands::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MyVPN");
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItemBuilder::with_id("open", "Open MyVPN").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit MyVPN").build(app)?;
    let menu = MenuBuilder::new(app).items(&[&open, &quit]).build()?;

    let _tray = TrayIconBuilder::with_id("main")
        .tooltip("MyVPN")
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn show_main<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}
