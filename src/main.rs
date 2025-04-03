pub mod config;
pub mod discord;
pub mod serial;

use std::{thread, time};

// use core::time;
use std::time::Duration;

use discord::worker::DiscordWorker;
use discord::client::DiscordClient;
// use serial::port::Port;
// use serial::worker::SerialWorker;
use config::config::Config;
use std::io::{self};

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#[cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let mut config = Config::new();
    config.load();

    let mut ds = DiscordClient::new(
        "713524519830028368".to_string(),
        Some("S8ngQYkWFytsdOsr0W1ULVlo9XQk2y".to_string()),
        "4Xqsf4ELABGEph3ZsmaaIp3Urr60Ikzp".to_string(),
        "https://www.mechardo3d.xyz/".to_string()
    );

    while !ds.is_connected(){
        ds.connect_loop();
    }


    for i in 0..1000{
        thread::sleep(time::Duration::from_millis(100));
        match ds.get_voice_settings(){
            Some((m, d)) => {
                println!("{} muted: {} | deafen: {}",i, m, d);
                // ds.set_voice_settings(!m, !d);
            }
            None => {
                println!("{}",i);
            }
        }
    }

    if false {
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
}
