mod backend;
mod config;
mod controller;

use controller::*;
use std::{
    sync::{Arc, Mutex},
    thread,
};

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#[cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    let controller = Arc::new(Mutex::new(Controller::new()));

    let controller_clone = controller.clone();
    let _jh = thread::spawn(move || loop {
        {
            controller_clone.lock().unwrap().controller_loop();
        }
    });

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
            controller::ds_set_voice_settings_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    // let mut config = Config::new();
    // config.load();

    // let mut ds = DiscordWorker::new();
    // ds.start(config.discord_access_token).unwrap();

    // let mut _sw = SerialWorker::new();
    // // sw.start(config.last_port_connected.clone()).unwrap();

    // let mut mute;
    // let mut deafen;

    // for _i in 0..1000 {
    //     config.discord_access_token = ds.get_config();
    //     config.save();

    //     let mut input = String::new();
    //     io::stdin().read_line(&mut input).unwrap();

    //     if let Some(first_char) = input.trim().chars().next() {
    //         (mute, deafen) = ds.get_voice_settings().unwrap();
    //         match first_char {
    //             'm' => {
    //                 mute = !mute;
    //                 ds.set_voice_settings(mute, deafen).unwrap();
    //             }
    //             'd' => {
    //                 deafen = !deafen;
    //                 ds.set_voice_settings(mute || deafen, deafen).unwrap();
    //             }
    //             'w' => {
    //                 ds.disconnect().unwrap();
    //             }
    //             'q' => {
    //                 break;
    //             }
    //             _ => {}
    //         }
    //     }

    //     // if sw.get_disconenct() {
    //     //     ds.disconnect().unwrap();
    //     // }
    // }

    // ds.stop().unwrap();
}
