use config::*;
use discord::discord_worker::{self, DiscordWorker};
use serial::serial_worker::SerialWorker;
use tauri::AppHandle;

use tracing::info;

pub struct Controller {
    discord_worker: DiscordWorker,
    serial_worker: SerialWorker,
    config: DSConfig,
}

impl Controller {
    pub async fn new() -> Self {
        let mut config = DSConfig::new();
        config.load();

        let mut discord_worker = DiscordWorker::new(
            config.discord_client_id.clone(),
            config.discord_secret_key.clone(),
            config.redirect_url.clone(),
            config.discord_access_token.clone(),
            config.discord_refresh_token.clone(),
        )
        .await;
        let mut serial_worker = SerialWorker::new();

        discord_worker.start().await.unwrap();
        serial_worker
            .start(config.last_port_connected.clone())
            .await
            .unwrap();

        Controller {
            discord_worker: discord_worker,
            serial_worker: serial_worker,
            config: config,
        }
    }

    pub async fn ds_set_voice_settings(&mut self, mute: bool, deaf: bool) {
        self.discord_worker
            .set_voice_settings(mute, deaf)
            .await
            .unwrap();
    }

    pub async fn controller_loop(&mut self, _app: &AppHandle) {
        // app.emit("DOWNLOAD_PROGRESS", "HOLA").unwrap();
        //TODO DW and SW logic
        if self.serial_worker.has_update() {}
        let discord_voice_settings = self.discord_worker.get_voice_settings().await;

        info!(
            "mute: {}   deaf:{}",
            discord_voice_settings.mute, discord_voice_settings.deafen
        );

        // if self.discord_worker.has_update() {
        //     if let Some(update) = self.discord_worker.get_update() {
        //         match update {
        //             DiscordUpdate::NewAccessToken(token) => {
        //                 self.config.discord_access_token = Some(token);
        //                 self.config.save();
        //             }
        //             DiscordUpdate::NewRefreshToken(token) => {
        //                 self.config.discord_refresh_token = Some(token);
        //                 self.config.save();
        //             }
        //             DiscordUpdate::NewDiscordVoiceSetting(mute, deaf) => {
        //                 //todo
        //                 // sw.set_voice_settings(mute, deaf);
        //                 info!("mute: {}   deaf:{}", mute, deaf);
        //             }
        //         }
        //     }
        // }
    }
}

pub mod commands {
    use super::Controller;
    use std::sync::{Arc, Mutex};
    use tauri::{AppHandle, State};
    use tracing::info;

    #[tauri::command]
    pub fn ds_set_voice_settings_command(
        mute: bool,
        deaf: bool,
        controller: State<'_, Arc<Mutex<Controller>>>,
    ) {
        info!("ds_set_voice_settings_command");
        controller.lock().unwrap().ds_set_voice_settings(mute, deaf);
    }

    #[tauri::command]
    pub fn controller_start(app: AppHandle, controller: State<'_, Arc<Mutex<Controller>>>) {
        let controller_clone = controller.inner().clone();
        std::thread::spawn(move || {
            loop {
                controller_clone.lock().unwrap().controller_loop(&app);
            }
        });
    }
}
