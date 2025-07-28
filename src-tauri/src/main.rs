use controller::commands;
use controller::Controller;

use std::sync::{Arc, Mutex};
use tracing::{debug, info};

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#[cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    let controller = Arc::new(Mutex::new(Controller::new()));

    tracing_subscriber::fmt().init();

    info!("Aplicación iniciada");
    debug!("Este es un log de debug");

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
