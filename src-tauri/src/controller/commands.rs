use crate::{controller::Controller, error::ControllerError};

use serial::messages::button::Button;
use serial::serial_message::SerialMessage;
use std::sync::Arc;
use std::time::SystemTime;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};
use tracing::debug;

const DISCORD_CONNECTION_STATUS_EVENT: &str = "DISCORD_CONNECTION_STATUS_EVENT";
const DISCORD_VOICE_SETTINGS_EVENT: &str = "DISCORD_VOICE_SETTINGS_EVENT";
const SERIAL_CONNECTION_STATUS_EVENT: &str = "SERIAL_CONNECTION_STATUS_EVENT";

const PERIODICAL_SERIAL_UPDATE: Duration = Duration::from_millis(100);

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
        .ds_set_voice_settings(mute || deaf, deaf)
        .await;
    Ok(())
}

#[tauri::command]
pub async fn controller_start(
    app: AppHandle,
    controller: State<'_, Arc<Mutex<Controller>>>,
) -> Result<(), &'static str> {
    debug!("Starting controller");
    let controller_clone = controller.inner().clone();
    let app_clone = app.clone();
    let jh: tokio::task::JoinHandle<Result<(), ControllerError>> =
        tokio::spawn(async move { background_loop(app_clone, controller_clone).await });
    controller.lock().await.backgroung_join_handle = Some(jh);
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
    let mut last_serial_update = SystemTime::now();

    debug!("Starting background loop");

    loop {
        let mut controller_lock = controller.lock().await;

        if !controller_lock.discord_worker.is_connected().await? {
            app.emit(DISCORD_CONNECTION_STATUS_EVENT, "false")?;
        }
        app.emit(DISCORD_CONNECTION_STATUS_EVENT, "true")?;

        if !controller_lock.serial_worker.is_connected().await? {
            app.emit(SERIAL_CONNECTION_STATUS_EVENT, "false")?;
        } else {
            app.emit(SERIAL_CONNECTION_STATUS_EVENT, "true")?;

            if SystemTime::now().duration_since(last_serial_update)? > PERIODICAL_SERIAL_UPDATE {
                last_serial_update = SystemTime::now();
                controller_lock
                    .serial_worker
                    .set_voice_settings(voice_settings.mute, voice_settings.deafen)
                    .await?;
            }
        }

        let pending_serial_messages = controller_lock.serial_worker.get_pending_messages().await?;

        for pending_message in pending_serial_messages {
            match pending_message {
                SerialMessage::Button(msg) => match msg.button {
                    Button::MuteButton => {
                        voice_settings.mute = !voice_settings.mute;
                        controller_lock
                            .discord_worker
                            .set_voice_settings(
                                voice_settings.mute || voice_settings.deafen,
                                voice_settings.deafen,
                            )
                            .await?;
                    }
                    Button::DeafenButton => {
                        voice_settings.deafen = !voice_settings.deafen;
                        controller_lock
                            .discord_worker
                            .set_voice_settings(
                                voice_settings.mute || voice_settings.deafen,
                                voice_settings.deafen,
                            )
                            .await?;
                    }
                    Button::DisconnectButton => {
                        controller_lock.discord_worker.disconnect().await?;
                    }
                },
                _ => { //TODO handle other msgs
                }
            }
        }

        let discord_voice_settings = controller_lock.discord_worker.get_voice_settings().await;

        if voice_settings != discord_voice_settings {
            debug!("Voice settings change detected:");
            voice_settings = discord_voice_settings;
            debug!(
                "Mute: {} Deafen: {}",
                voice_settings.mute, voice_settings.deafen
            );
            // controller_lock
            //     .serial_worker
            //     .set_voice_settings(voice_settings.mute, voice_settings.deafen)
            //     .await?;
        }

        // Store configs
        let access_token = controller_lock.discord_worker.get_access_token().await?;
        let refresh_token = controller_lock.discord_worker.get_refresh_token().await?;

        controller_lock
            .config
            .update_tokens(access_token, refresh_token)
            .await;

        let last_port_used = controller_lock.serial_worker.get_port_name().await?;
        controller_lock
            .config
            .update_last_used_port(last_port_used)
            .await;

        app.emit(
            DISCORD_VOICE_SETTINGS_EVENT,
            format!(
                "{{\"mute\":{},\"deafen\":{}}}",
                voice_settings.mute, voice_settings.deafen,
            ),
        )?;
        sleep(Duration::from_millis(100)).await;
    }
}
