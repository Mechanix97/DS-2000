use controller::commands;
use controller::controller::Controller;
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;
use tracing::{debug, info};

#[cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
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
        .plugin(tauri_plugin_shell::init())
        .manage(controller.clone())
        .manage(shutdown_complete.clone())
        .invoke_handler(tauri::generate_handler![
            commands::controller_start,
            commands::ds_set_voice_settings_command
        ])
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                debug!("Hiding app");
                api.prevent_close();
                window.hide().unwrap();
            }
            _ => {}
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
                .on_tray_icon_event(|tray, event| match event {
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } => {
                        debug!("left click pressed and released");
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
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
