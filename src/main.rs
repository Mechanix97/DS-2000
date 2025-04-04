pub mod config;
pub mod discord;
pub mod serial;

use std::{thread, time};

// use core::time;
use std::time::Duration;

use discord::client::DiscordClient;
use discord::worker::DiscordWorker;
use serial::worker::SerialWorker;
use config::config::Config;
use std::io::{self};

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#[cfg_attr(not(debug_assertions), windows_subsystem = "windows")]


fn main() {
    let mut config = Config::new();
    config.load();

    let mut ds = DiscordWorker::new();
    ds.start(config.discord_access_token).unwrap();

    let mut sw = SerialWorker::new();
    sw.start(config.last_port_connected.clone()).unwrap();
    


    let mut mute = false;
    let mut deafen = false;
  

    for _i in 0..1000{
        config.discord_access_token = ds.get_config();
        config.save();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
    
        if let Some(first_char) = input.trim().chars().next(){
            (mute, deafen)=ds.get_voice_settings().unwrap();
            match first_char {
                'm' => {
                    mute = !mute;
                }
                'd' =>{
                    
                    deafen = !deafen;
                }
                'w' => {
                    ds.disconnect().unwrap();
                }
                'q' => {
                    break;
                }
                _ => {

                }
            }
        }

        if sw.get_disconenct(){
            ds.disconnect().unwrap();          
        }
    }

    // for _i in 0..1000{
    //     config.discord_access_token = ds.get_config();
    //     config.save();

    //     let mut input = String::new();
    //     io::stdin().read_line(&mut input).unwrap();
    
    //     // Obtener el primer carácter si existe
    //     if let Some(first_char) = input.trim().chars().next(){
    //         (mute, deafen)=ds.get_voice_settings().unwrap();
    //         match first_char {
    //             'm' => {
    //                 mute = !mute;
    //             }
    //             'd' =>{
                    
    //                 deafen = !deafen;
    //             }
    //             'w' => {
    //                 ds.disconnect().unwrap();
    //             }
    //             'q' => {
    //                 break;
    //             }
    //             _ => {

    //             }
    //         }
    //     }

        // ds.set_voice_settings(mute || deafen, deafen).unwrap();
        
    // }

    ds.stop().unwrap();

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
