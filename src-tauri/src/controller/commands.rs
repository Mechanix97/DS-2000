use crate::{controller::Controller, error::ControllerError};

use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;
use tracing::debug;

#[tauri::command]
pub async fn ds_set_voice_settings_command(
    mute: bool,
    deaf: bool,
    controller: State<'_, Arc<Mutex<Controller>>>,
) -> Result<(), &'static str> {
    debug!("ds_set_voice_settings_command");
    controller
        .lock()
        .await
        .ds_set_voice_settings(mute, deaf)
        .await;
    Ok(())
}

#[tauri::command]
pub async fn controller_start(
    app: AppHandle,
    controller: State<'_, Arc<Mutex<Controller>>>,
) -> Result<(), &'static str> {
    debug!("controller start");
    let controller_clone = controller.inner().clone();
    let app_clone = app.clone();
    tokio::spawn(async move { background_loop(app_clone, controller_clone).await });
    Ok(())
}

async fn background_loop(
    app: AppHandle,
    controller: Arc<Mutex<Controller>>,
) -> Result<(), ControllerError> {
    let mut voice_settings = controller
        .lock()
        .await
        .discord_worker
        .get_voice_settings()
        .await;

    debug!("Starting background loop");

    loop {
        let controller_lock = controller.lock().await;
        let discord_voice_settings = controller_lock.discord_worker.get_voice_settings().await;

        if voice_settings != discord_voice_settings {
            debug!("Voice settings change detected:");
            debug!(
                "Mute: {} Deafen: {}",
                voice_settings.mute, voice_settings.deafen
            );
            voice_settings = discord_voice_settings;
            // controller_lock
            //     .serial_worker
            //     .set_voice_settings(voice_settings.mute, voice_settings.deafen);
            app.emit("DOWNLOAD_PROGRESS", "HOLA")?;
        }
    }
}
