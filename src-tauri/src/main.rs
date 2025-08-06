use controller::commands;
use controller::controller::Controller;

use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#[cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("App started");

    let controller = Arc::new(Mutex::new(Controller::new().await));
    controller
        .lock()
        .await
        .start()
        .await
        .expect("Controller couldn't start");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(controller)
        .invoke_handler(tauri::generate_handler![
            commands::controller_start,
            commands::ds_set_voice_settings_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
