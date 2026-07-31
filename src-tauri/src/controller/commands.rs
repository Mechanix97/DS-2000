use crate::controller::Controller;
use crate::coordinator::UiRefreshHandle;
use common::rgb_update::{LedRgb, RGBConfig, RGBMode};
use config::credentials::URL_DISCORD_SETUP_GUIDE;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, State};
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

/// Version the application was built with.
///
/// Read from the Tauri package info so there is one source of truth. The About tab used to
/// carry a hardcoded "1.0" while Cargo.toml said 0.1.0 and tauri.conf.json said 0.1.1.
#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
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

/// Lighting configuration as the frontend sends it.
///
/// The mode travels as a name rather than the index of a `<select>`. Coupling it to the option
/// order meant reordering the dropdown silently changed what the device did — and they had in
/// fact drifted apart: the UI's second option read "Respiración" while the backend mapped it to
/// `Fixed`, and the third read "Fijo" while mapping to `Wave`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RgbRequest {
    pub mode: RgbModeName,
    pub brightness: u8,
    pub led1: LedRgb,
    pub led2: LedRgb,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum RgbModeName {
    Cycle,
    Fixed,
    Wave,
}

impl From<RgbRequest> for RGBConfig {
    fn from(request: RgbRequest) -> Self {
        let RgbRequest {
            mode,
            brightness,
            led1,
            led2,
        } = request;

        RGBConfig {
            brightness,
            rgb_mode: match mode {
                RgbModeName::Cycle => RGBMode::Cycle,
                RgbModeName::Fixed => RGBMode::Fixed { led1, led2 },
                RgbModeName::Wave => RGBMode::Wave { led1, led2 },
            },
        }
    }
}

#[tauri::command]
pub async fn serial_set_rgb(
    request: RgbRequest,
    controller: State<'_, Arc<Mutex<Controller>>>,
) -> Result<(), String> {
    let mut update = RGBConfig::from(request);
    // 0xFF terminates a frame on the wire, so it cannot appear inside one.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn led(red: u8, green: u8, blue: u8) -> LedRgb {
        LedRgb { red, green, blue }
    }

    fn request(mode: &str) -> RgbRequest {
        serde_json::from_str(&format!(
            r#"{{"mode":"{mode}","brightness":128,
                 "led1":{{"red":1,"green":2,"blue":3}},
                 "led2":{{"red":4,"green":5,"blue":6}}}}"#
        ))
        .expect("deserialises")
    }

    #[test]
    fn each_mode_name_maps_to_its_own_variant() {
        // Guards the coupling that used to exist through the dropdown's option order.
        assert_eq!(
            RGBConfig::from(request("fixed")).rgb_mode,
            RGBMode::Fixed {
                led1: led(1, 2, 3),
                led2: led(4, 5, 6)
            }
        );
        assert_eq!(
            RGBConfig::from(request("wave")).rgb_mode,
            RGBMode::Wave {
                led1: led(1, 2, 3),
                led2: led(4, 5, 6)
            }
        );
        assert_eq!(RGBConfig::from(request("cycle")).rgb_mode, RGBMode::Cycle);
    }

    #[test]
    fn an_unknown_mode_is_rejected_rather_than_defaulted() {
        assert!(serde_json::from_str::<RgbRequest>(r#"{"mode":"breathing","brightness":1,"led1":{"red":0,"green":0,"blue":0},"led2":{"red":0,"green":0,"blue":0}}"#).is_err());
    }

    #[test]
    fn brightness_and_colours_never_carry_the_frame_delimiter() {
        // 0xFF ends a frame, so a payload byte of 255 would cut it short.
        let mut config = RGBConfig::from(request("fixed"));
        config.brightness = 255;
        config.rgb_mode = RGBMode::Fixed {
            led1: led(255, 255, 255),
            led2: led(255, 0, 255),
        };
        config.check_255();

        assert_eq!(config.brightness, 254);
        let RGBMode::Fixed { led1, led2 } = config.rgb_mode else {
            panic!("mode preserved");
        };
        assert_eq!(led1, led(254, 254, 254));
        assert_eq!(led2, led(254, 0, 254));
    }
}
