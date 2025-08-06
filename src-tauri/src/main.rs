use controller::commands;
use controller::controller::Controller;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;
use tracing::info;

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
                info!("Closing app");
                api.prevent_close();
                let app = window.app_handle().clone();
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
            _ => {}
        })
        .setup(|app| {
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
