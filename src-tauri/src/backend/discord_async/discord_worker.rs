use crate::discord_state::DiscordState;
use crate::discord_state::DiscordStateHandler;
use crate::discord_state::DiscordVoiceSettings;
use crate::discord_state::InCallMessage;
use crate::discord_state::InMessage;
use crate::discord_state::OutMessage;
use crate::error::DiscordError;
use crate::ipc::DiscordConnectionState;

use std::sync::Arc;
use tokio::sync::Mutex;

pub struct DiscordWorker {
    discord_handler: DiscordStateHandler,
    voice_settings: Arc<Mutex<DiscordVoiceSettings>>,
}

impl DiscordWorker {
    pub async fn new(
        client_id: String,
        client_secret: String,
        redirect_url: String,
        access_token: Option<String>,
        refresh_token: Option<String>,
    ) -> Self {
        let voice_settings = Arc::new(Mutex::new(DiscordVoiceSettings {
            mute: false,
            deafen: false,
        }));

        let discord_handler = DiscordState::spawn(
            client_id,
            client_secret,
            redirect_url,
            access_token,
            refresh_token,
            voice_settings.clone(),
        )
        .await;

        Self {
            discord_handler,
            voice_settings,
        }
    }

    pub async fn start(&mut self) -> Result<(), DiscordError> {
        self.discord_handler
            .cast(InMessage::Fetch)
            .await
            .map_err(|e| DiscordError::GenServerError(e))
    }

    pub async fn get_voice_settings(&self) -> DiscordVoiceSettings {
        let lock = self.voice_settings.lock().await;
        (*lock).clone()
    }

    pub async fn set_voice_settings(
        &mut self,
        mute: bool,
        deafen: bool,
    ) -> Result<(), DiscordError> {
        self.discord_handler
            .cast(InMessage::SetVoiceSetting(mute, deafen))
            .await
            .map_err(|e| DiscordError::GenServerError(e))
    }

    pub async fn is_connected(&mut self) -> Result<bool, DiscordError> {
        let st = self
            .discord_handler
            .call(InCallMessage::DiscordStatus)
            .await
            .map_err(|e| DiscordError::GenServerError(e))?;
        if st == OutMessage::DiscordStatus(DiscordConnectionState::Authenticated) {
            return Ok(true);
        }
        Ok(false)
    }
}
