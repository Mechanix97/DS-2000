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
        let st: OutMessage = self
            .discord_handler
            .call(InCallMessage::DiscordStatus)
            .await
            .map_err(|e| DiscordError::GenServerError(e))?;
        if st == OutMessage::DiscordStatus(DiscordConnectionState::Authenticated) {
            return Ok(true);
        }
        Ok(false)
    }

    pub async fn disconnect(&mut self) -> Result<(), DiscordError> {
        self.discord_handler
            .cast(InMessage::DisconnectChannel)
            .await
            .map_err(|e| DiscordError::GenServerError(e))
    }
}

#[cfg(test)]
mod tests {
    use super::DiscordWorker;
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    use std::path::PathBuf;
    use tokio::time::{Duration, sleep};

    fn load_env_file() {
        let env_file_path = PathBuf::from("../../../../discord.env");

        let reader = BufReader::new(File::open(env_file_path).unwrap());

        for line in reader.lines() {
            let line = line.unwrap();
            if line.starts_with("#") {
                continue;
            };
            match line.split_once('=') {
                Some((key, value)) => unsafe { std::env::set_var(key, value) },
                None => continue,
            };
        }
    }
    #[tokio::test]
    async fn test_discord_worker_connection() {
        load_env_file();

        let client_id = std::env::var("DISCORD_CLIENT_ID").unwrap();
        let client_secret = std::env::var("DISCORD_SECRET_KEY").unwrap();
        let redirect_url = "https://www.mechardo3d.xyz/".to_string();

        let mut discord_worker =
            DiscordWorker::new(client_id, client_secret, redirect_url, None, None).await;

        discord_worker.start().await.unwrap();

        while !discord_worker.is_connected().await.unwrap() {}

        discord_worker
            .set_voice_settings(true, false)
            .await
            .unwrap();
        sleep(Duration::from_secs(1)).await;

        let vs = discord_worker.get_voice_settings().await;

        assert!(vs.mute);
        assert!(!vs.deafen);

        discord_worker.set_voice_settings(true, true).await.unwrap();
        sleep(Duration::from_secs(1)).await;

        let vs = discord_worker.get_voice_settings().await;

        assert!(vs.mute);
        assert!(vs.deafen);

        discord_worker
            .set_voice_settings(false, false)
            .await
            .unwrap();
        sleep(Duration::from_secs(1)).await;

        let vs = discord_worker.get_voice_settings().await;

        assert!(!vs.mute);
        assert!(!vs.deafen);

        discord_worker.disconnect().await.unwrap();
        sleep(Duration::from_secs(1)).await;
    }
}
