mod backend;
mod config;
mod controller;

use controller::*;
use std::sync::{Arc, Mutex};

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#[cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    let controller = Arc::new(Mutex::new(Controller::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(controller)
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            controller::controller_start,
            controller::ds_set_voice_settings_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
