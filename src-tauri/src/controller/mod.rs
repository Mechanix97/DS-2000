use crate::backend::discord::discord_worker::DiscordWorker;
use crate::backend::serial::serial_worker::SerialWorker;
use crate::config::*;

use std::sync::Mutex;
use tauri::State;

pub struct Controller {
    discord_worker: DiscordWorker,
    serial_worker: SerialWorker,
    config: DSConfig
}

impl Controller{
    pub fn new() -> Self {
        let mut discord_worker = DiscordWorker::new();
        let mut serial_worker = SerialWorker::new();
        let mut config = DSConfig::new();

        config.load();
        discord_worker.start(config.discord_access_token.clone()).unwrap();
        // serial_worker.start(config.last_port_connected.clone()).unwrap();
        
        Controller {
            discord_worker: discord_worker,
            serial_worker: serial_worker,
            config: config
        }
    }

    pub fn save(&mut self) {
        self.config.save();
    }

    pub fn ds_set_voice_settings(&mut self, mute: bool, deaf: bool){
        self.discord_worker.set_voice_settings(mute, deaf);
    }
}


#[tauri::command]
pub fn hacer_algo(nombre: String, controller: State<'_, Mutex<Controller>>) {
    println!("Hola, {}!", nombre);

    controller.lock().unwrap().save();
    // Ejemplo: podrías usar ctrl.discord_worker o lo que necesites
}

#[tauri::command]
pub fn ds_set_voice_settings_command(mute: bool, deaf: bool, controller: State<'_, Mutex<Controller>>) {
    controller.lock().unwrap().ds_set_voice_settings(mute, deaf);
}
