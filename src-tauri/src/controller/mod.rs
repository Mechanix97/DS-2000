use crate::backend::discord::discord_worker::{DiscordUpdate, DiscordWorker};
use crate::backend::serial::serial_worker::SerialWorker;
use crate::config::*;

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

pub struct Controller {
    discord_worker: DiscordWorker,
    serial_worker: SerialWorker,
    config: DSConfig,
}

impl Controller {
    pub fn new() -> Self {
        let mut discord_worker = DiscordWorker::new();
        let mut serial_worker = SerialWorker::new();
        let mut config = DSConfig::new();

        config.load();
        discord_worker.start(config.clone()).unwrap();
        serial_worker.start(config.last_port_connected.clone()).unwrap();

        Controller {
            discord_worker: discord_worker,
            serial_worker: serial_worker,
            config: config,
        }
    }

    pub fn ds_set_voice_settings(&mut self, mute: bool, deaf: bool) {
        self.discord_worker.set_voice_settings(mute, deaf).unwrap();
    }

    pub fn controller_loop(&mut self, app: &AppHandle) {
        app.emit("DOWNLOAD_PROGRESS", "HOLA").unwrap();
        //TODO DW and SW logic
        if self.serial_worker.has_update() {}
        if self.discord_worker.has_update() {
            if let Some(update) = self.discord_worker.get_update() {
                match update {
                    DiscordUpdate::NewAccessToken(token) => {
                        self.config.discord_access_token = Some(token);
                        self.config.save();
                    }
                    DiscordUpdate::NewRefreshToken(token) => {
                        self.config.discord_refresh_token = Some(token);
                        self.config.save();
                    }
                    DiscordUpdate::NewDiscordVoiceSetting(mute, deaf) => {
                        //todo
                        // sw.set_voice_settings(mute, deaf);
                        println!("mute: {}   deaf:{}", mute, deaf);
                    }
                }
            }
        }
    }
}

#[tauri::command]
pub fn ds_set_voice_settings_command(
    mute: bool,
    deaf: bool,
    controller: State<'_, Arc<Mutex<Controller>>>,
) {
    controller.lock().unwrap().ds_set_voice_settings(mute, deaf);
}

#[tauri::command]
pub fn controller_start(
    app: AppHandle,
    controller: State<'_, Arc<Mutex<Controller>>>,
) {
    let conttoller_clone = controller.inner().clone();
    std::thread::spawn(move || {
        loop{
            conttoller_clone.lock().unwrap().controller_loop(&app);
        }
    });
}
