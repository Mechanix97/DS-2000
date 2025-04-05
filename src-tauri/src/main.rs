mod backend;
mod config;

use backend::discord::discord_worker::*;
use backend::serial::serial_worker::*;
use config::*;

use std::io::{self};


#[tauri::command]
fn hacer_algo(nombre: String) {
    println!("Hola, {}!", nombre);
}


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
        .invoke_handler(tauri::generate_handler![hacer_algo])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    let mut config = Config::new();
    config.load();

    let mut ds = DiscordWorker::new();
    ds.start(config.discord_access_token).unwrap();

    let mut _sw = SerialWorker::new();
    // sw.start(config.last_port_connected.clone()).unwrap();

    let mut mute;
    let mut deafen;

    for _i in 0..1000 {
        config.discord_access_token = ds.get_config();
        config.save();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        if let Some(first_char) = input.trim().chars().next() {
            (mute, deafen) = ds.get_voice_settings().unwrap();
            match first_char {
                'm' => {
                    mute = !mute;
                    ds.set_voice_settings(mute, deafen).unwrap();
                }
                'd' => {
                    deafen = !deafen;
                    ds.set_voice_settings(mute || deafen, deafen).unwrap();
                }
                'w' => {
                    ds.disconnect().unwrap();
                }
                'q' => {
                    break;
                }
                _ => {}
            }
        }

        // if sw.get_disconenct() {
        //     ds.disconnect().unwrap();
        // }
    }

    ds.stop().unwrap();
}
