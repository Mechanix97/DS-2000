use crate::controller::Controller;
use crate::coordinator::UiRefreshHandle;
use common::rgb_update::{LedRgb, RGBConfig, RGBMode};
use config::credentials::URL_DISCORD_SETUP_GUIDE;

use serde::Serialize;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;
use tracing::debug;

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
    let controller = controller.lock().await;

    let client_id = controller.config.discord_client_id().await;
    let has_client_secret = controller.has_discord_credentials().await;
    let connected = controller.discord_worker.is_connected();

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
pub async fn controller_start(ui_refresh: State<'_, UiRefreshHandle>) -> Result<(), String> {
    // The coordinator is already running by the time the frontend loads; the workers were started
    // before the window existed. All this does now is ask for the current state, which the
    // frontend needs because nothing was emitted while there was no window to receive it.
    debug!("Frontend ready, resending the current state");
    let _ = ui_refresh.send(());
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
