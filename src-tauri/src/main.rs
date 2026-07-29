#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use controller::commands;
use controller::controller::Controller;
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;
use tracing::{debug, info};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("App started");

    let controller = Arc::new(Mutex::new(Controller::new().await));
    let shutdown_complete = Arc::new(Mutex::new(false));

    controller
        .lock()
        .await
        .start()
        .await
        .expect("Controller couldn't start");

    tauri::Builder::default()
        // TODO: add a single-instance guard. Without one, a second launch fights the first for
        // the serial port and the Discord pipe. The `tauri-plugin-single-instance` dependency
        // was dropped while the feature stays unimplemented; re-add it when picking this up.
        .plugin(tauri_plugin_shell::init())
        .manage(controller.clone())
        .manage(shutdown_complete.clone())
        .invoke_handler(tauri::generate_handler![
            commands::controller_start,
            commands::ds_set_voice_settings_command,
            commands::serial_set_rgb,
            commands::discord_credentials_status,
            commands::discord_set_credentials,
            commands::discord_clear_credentials,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                debug!("Hiding app");
                api.prevent_close();
                window.hide().unwrap();
            }
        })
        .setup(|app| {
            let quit_i = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Abrir DS2000", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            TrayIconBuilder::new()
                .menu(&menu)
                .icon(app.default_window_icon().unwrap().clone())
                .on_menu_event(|app_handle, event| match event.id.as_ref() {
                    "quit" => {
                        let app = app_handle.clone();
                        let controller = app.state::<Arc<Mutex<Controller>>>().inner().clone();
                        let shutdown_complete = app.state::<Arc<Mutex<bool>>>().inner().clone();
                        tauri::async_runtime::spawn(async move {
                            controller
                                .lock()
                                .await
                                .shutdown()
                                .await
                                .expect("Error shutting down controller");
                            *shutdown_complete.lock().await = true;
                            app.emit_to("main", "shutdown-complete", ()).unwrap();
                        });
                    }
                    "show" => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        debug!("left click pressed and released");
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;
            let app_handle = app.handle().clone();
            let shutdown_complete = app.state::<Arc<Mutex<bool>>>().inner().clone();
            std::thread::spawn(move || {
                while !*shutdown_complete.blocking_lock() {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                info!("Shutdown completed, exiting app");
                app_handle.exit(0);
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
