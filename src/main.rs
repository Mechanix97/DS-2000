pub mod config;
pub mod discord;
pub mod serial;

use std::{thread, time};

// use core::time;
use std::time::Duration;

use discord::worker::DiscordWorker;
// use serial::port::Port;
// use serial::worker::SerialWorker;
use config::config::Config;
use std::io::{self};

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#[cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
