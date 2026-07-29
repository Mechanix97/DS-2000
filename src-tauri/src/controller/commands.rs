use crate::{controller::Controller, error::ControllerError};
use common::rgb_update::{LedRgb, RGBConfig, RGBMode};
use config::credentials::URL_DISCORD_SETUP_GUIDE;

use serde::Serialize;
use serial::messages::button::Button;
use serial::serial_message::SerialMessage;
use std::sync::Arc;
use std::time::SystemTime;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};
use tracing::{debug, warn};

const DISCORD_CONNECTION_STATUS_EVENT: &str = "DISCORD_CONNECTION_STATUS_EVENT";
const DISCORD_VOICE_SETTINGS_EVENT: &str = "DISCORD_VOICE_SETTINGS_EVENT";
const SERIAL_CONNECTION_STATUS_EVENT: &str = "SERIAL_CONNECTION_STATUS_EVENT";

const PERIODICAL_SERIAL_UPDATE: Duration = Duration::from_millis(100);

/// What the Discord tab needs to render itself.
///
/// The client secret is deliberately absent: once stored it is never handed back to the
/// frontend, only replaced.
#[derive(Serialize)]
pub struct DiscordCredentialsStatus {
    pub client_id: Option<String>,
    pub has_client_secret: bool,
    pub connected: bool,
    pub setup_guide_url: &'static str,
    pub redirect_uri: &'static str,
}

#[derive(Serialize)]
struct VoiceSettingsPayload {
    mute: bool,
    deafen: bool,
}

#[tauri::command]
pub async fn ds_set_voice_settings_command(
    mute: bool,
    deaf: bool,
    controller: State<'_, Arc<Mutex<Controller>>>,
) -> Result<(), String> {
    debug!("ds_set_voice_settings_command");
    // Deafening implies muting: Discord will not accept a deafened-but-unmuted state.
    controller
        .lock()
        .await
        .ds_set_voice_settings(mute || deaf, deaf)
        .await
        .map_err(|err| err.to_string())
}

/// Reports whether a Discord application is configured, so the UI can decide whether to open on
/// the Discord tab and what to show there.
#[tauri::command]
pub async fn discord_credentials_status(
    controller: State<'_, Arc<Mutex<Controller>>>,
) -> Result<DiscordCredentialsStatus, String> {
    let mut controller = controller.lock().await;

    let client_id = controller.config.discord_client_id().await;
    let has_client_secret = controller.has_discord_credentials().await;
    let connected = controller
        .discord_worker
        .is_connected()
        .await
        .unwrap_or(false);

    Ok(DiscordCredentialsStatus {
        client_id,
        has_client_secret,
        connected,
        setup_guide_url: URL_DISCORD_SETUP_GUIDE,
        redirect_uri: config::credentials::DISCORD_REDIRECT_URI,
    })
}

/// Stores the Discord application credentials entered by the user and reconnects.
#[tauri::command]
pub async fn discord_set_credentials(
    client_id: String,
    client_secret: String,
    controller: State<'_, Arc<Mutex<Controller>>>,
) -> Result<(), String> {
    let client_id = client_id.trim();
    let client_secret = client_secret.trim();

    if client_id.is_empty() || client_secret.is_empty() {
        return Err("Both the Client ID and the Client Secret are required".to_owned());
    }
    if !client_id.chars().all(|c| c.is_ascii_digit()) {
        return Err("The Client ID should be numeric — copy it from OAuth2 → Client ID".to_owned());
    }

    controller
        .lock()
        .await
        .set_discord_credentials(client_id, client_secret)
        .await
        .map_err(|err| err.to_string())
}

/// Forgets the configured Discord application.
#[tauri::command]
pub async fn discord_clear_credentials(
    controller: State<'_, Arc<Mutex<Controller>>>,
) -> Result<(), String> {
    controller
        .lock()
        .await
        .clear_discord_credentials()
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn controller_start(
    app: AppHandle,
    controller: State<'_, Arc<Mutex<Controller>>>,
) -> Result<(), String> {
    debug!("Starting controller");
    let controller_clone = controller.inner().clone();
    let app_clone = app.clone();
    let handle = tokio::spawn(async move { background_loop(app_clone, controller_clone).await });
    controller.lock().await.background_join_handle = Some(handle);
    Ok(())
}

// TODO: collapse these into a single `RGBConfig` payload deserialized from the frontend, which
// also removes the mode-index coupling between `index.html` and this match.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn serial_set_rgb(
    mode: u8,
    brightness: u8,
    led1_red: u8,
    led1_green: u8,
    led1_blue: u8,
    led2_red: u8,
    led2_green: u8,
    led2_blue: u8,
    controller: State<'_, Arc<Mutex<Controller>>>,
) -> Result<(), String> {
    let led1 = LedRgb {
        red: led1_red,
        green: led1_green,
        blue: led1_blue,
    };
    let led2 = LedRgb {
        red: led2_red,
        green: led2_green,
        blue: led2_blue,
    };

    let mut update = match mode {
        0 => RGBConfig {
            brightness,
            rgb_mode: RGBMode::Cycle,
        },
        1 => RGBConfig {
            brightness,
            rgb_mode: RGBMode::Fixed { led1, led2 },
        },
        2 => RGBConfig {
            brightness,
            rgb_mode: RGBMode::Wave { led1, led2 },
        },
        _ => return Err("Invalid RGB mode".to_owned()),
    };
    update.check_255();

    let mut controller = controller.lock().await;
    controller.config.update_rgb(&update).await;
    controller
        .serial_worker
        .set_rgb_config(&update)
        .await
        .map_err(|err| err.to_string())?;

    debug!("RGB update: {update:?}");
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

        let discord_connected = controller_lock.discord_worker.is_connected().await?;
        app.emit(
            DISCORD_CONNECTION_STATUS_EVENT,
            discord_connected.to_string(),
        )?;

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
                        push_voice_settings(&mut controller_lock, &voice_settings).await?;
                    }
                    Button::DeafenButton => {
                        voice_settings.deafen = !voice_settings.deafen;
                        push_voice_settings(&mut controller_lock, &voice_settings).await?;
                    }
                    Button::DisconnectButton => {
                        controller_lock.discord_worker.disconnect().await?;
                    }
                },
                other => debug!("Unhandled serial message: {other:?}"),
            }
        }

        let discord_voice_settings = controller_lock.discord_worker.get_voice_settings().await;

        if voice_settings != discord_voice_settings {
            voice_settings = discord_voice_settings;
            debug!(
                "Voice settings changed — mute: {} deafen: {}",
                voice_settings.mute, voice_settings.deafen
            );
        }

        let access_token = controller_lock.discord_worker.get_access_token().await?;
        let refresh_token = controller_lock.discord_worker.get_refresh_token().await?;
        if let Err(err) = controller_lock
            .config
            .update_tokens(access_token, refresh_token)
            .await
        {
            warn!("Could not persist the Discord tokens: {err}");
        }

        let last_port_used = controller_lock.serial_worker.get_port_name().await?;
        controller_lock
            .config
            .update_last_used_port(last_port_used)
            .await;

        app.emit(
            DISCORD_VOICE_SETTINGS_EVENT,
            serde_json::to_string(&VoiceSettingsPayload {
                mute: voice_settings.mute,
                deafen: voice_settings.deafen,
            })
            .map_err(|err| ControllerError::GenericError(err.to_string()))?,
        )?;

        drop(controller_lock);
        sleep(Duration::from_millis(100)).await;
    }
}

async fn push_voice_settings(
    controller: &mut Controller,
    voice_settings: &discord::discord_state::DiscordVoiceSettings,
) -> Result<(), ControllerError> {
    // Deafening implies muting, mirroring what Discord itself enforces.
    controller
        .discord_worker
        .set_voice_settings(
            voice_settings.mute || voice_settings.deafen,
            voice_settings.deafen,
        )
        .await?;
    Ok(())
}
