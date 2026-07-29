use crate::credentials::DiscordCredentials;
use crate::discord_state::{
    DiscordState, DiscordStateHandler, DiscordVoiceSettings, InCallMessage, InMessage, OutMessage,
};
use crate::error::DiscordError;
use crate::ipc::DiscordConnectionState;

use std::sync::Arc;
use tokio::sync::Mutex;

pub struct DiscordWorker {
    discord_handler: DiscordStateHandler,
    /// Shared with the state machine so reading the current voice state does not need a
    /// round-trip through the actor mailbox.
    voice_settings: Arc<Mutex<DiscordVoiceSettings>>,
}

impl DiscordWorker {
    /// Creates the worker. `credentials` is `None` until the user registers a Discord
    /// application; the worker stays idle in that case and starts on [`Self::set_credentials`].
    pub async fn new(credentials: Option<DiscordCredentials>) -> Self {
        let voice_settings = Arc::new(Mutex::new(DiscordVoiceSettings::default()));
        let discord_handler = DiscordState::spawn(credentials, voice_settings.clone()).await;

        Self {
            discord_handler,
            voice_settings,
        }
    }

    /// Begins connecting. Safe to call with no credentials: it is then a no-op.
    pub async fn start(&mut self) -> Result<(), DiscordError> {
        self.discord_handler
            .cast(InMessage::Connect)
            .await
            .map_err(DiscordError::GenServerError)
    }

    /// Applies credentials entered at runtime. Passing `None` stops the connection.
    pub async fn set_credentials(
        &mut self,
        credentials: Option<DiscordCredentials>,
    ) -> Result<(), DiscordError> {
        self.discord_handler
            .cast(InMessage::SetCredentials(Box::new(credentials)))
            .await
            .map_err(DiscordError::GenServerError)
    }

    pub async fn get_voice_settings(&self) -> DiscordVoiceSettings {
        *self.voice_settings.lock().await
    }

    pub async fn set_voice_settings(
        &mut self,
        mute: bool,
        deafen: bool,
    ) -> Result<(), DiscordError> {
        self.discord_handler
            .cast(InMessage::SetVoiceSetting(mute, deafen))
            .await
            .map_err(DiscordError::GenServerError)
    }

    pub async fn is_connected(&mut self) -> Result<bool, DiscordError> {
        let status: OutMessage = self
            .discord_handler
            .call(InCallMessage::DiscordStatus)
            .await
            .map_err(DiscordError::GenServerError)?;
        Ok(status == OutMessage::DiscordStatus(DiscordConnectionState::Authenticated))
    }

    pub async fn disconnect(&mut self) -> Result<(), DiscordError> {
        self.discord_handler
            .cast(InMessage::DisconnectChannel)
            .await
            .map_err(DiscordError::GenServerError)
    }

    pub async fn get_access_token(&mut self) -> Result<Option<String>, DiscordError> {
        let message: OutMessage = self
            .discord_handler
            .call(InCallMessage::AccessToken)
            .await
            .map_err(DiscordError::GenServerError)?;
        let OutMessage::AccessToken(token) = message else {
            return Ok(None);
        };
        Ok(token)
    }

    pub async fn get_refresh_token(&mut self) -> Result<Option<String>, DiscordError> {
        let message: OutMessage = self
            .discord_handler
            .call(InCallMessage::RefreshToken)
            .await
            .map_err(DiscordError::GenServerError)?;
        let OutMessage::RefreshToken(token) = message else {
            return Ok(None);
        };
        Ok(token)
    }

    pub async fn shutdown(&mut self) -> Result<(), DiscordError> {
        self.discord_handler
            .call(InCallMessage::Shutdown)
            .await
            .map_err(DiscordError::GenServerError)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    use std::path::PathBuf;
    use tokio::time::{Duration, sleep};

    #[tokio::test]
    async fn a_worker_without_credentials_reports_itself_disconnected() {
        let mut worker = DiscordWorker::new(None).await;

        worker.start().await.expect("start is accepted");

        assert!(!worker.is_connected().await.expect("status is readable"));
        assert!(worker.get_access_token().await.expect("readable").is_none());
    }

    #[tokio::test]
    async fn an_idle_worker_never_reports_a_connection_on_its_own() {
        // Guards the "stay parked without credentials" behaviour: a regression here would mean
        // burning CPU retrying a connection that cannot succeed.
        let mut worker = DiscordWorker::new(None).await;
        worker.start().await.expect("start is accepted");

        sleep(Duration::from_millis(300)).await;

        assert!(!worker.is_connected().await.expect("status is readable"));
        assert_eq!(
            worker.get_voice_settings().await,
            DiscordVoiceSettings::default()
        );
    }

    /// Reads the developer's own Discord application credentials, the same way a user supplies
    /// theirs through the UI.
    fn credentials_from_env_file() -> DiscordCredentials {
        let file = File::open(PathBuf::from("../../../../discord.env")).expect("discord.env");
        for line in BufReader::new(file).lines() {
            let line = line.expect("readable line");
            if line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                unsafe { std::env::set_var(key, value) };
            }
        }

        DiscordCredentials::new(
            std::env::var("DISCORD_CLIENT_ID").expect("DISCORD_CLIENT_ID"),
            std::env::var("DISCORD_SECRET_KEY").expect("DISCORD_SECRET_KEY"),
            "http://localhost/".to_owned(),
        )
    }

    #[tokio::test]
    #[ignore = "needs a running Discord client and a local discord.env; run with --ignored"]
    async fn credentials_supplied_at_runtime_drive_a_full_connection() {
        let mut worker = DiscordWorker::new(None).await;
        worker.start().await.expect("start is accepted");

        // Nothing should happen until credentials arrive: this is the state a fresh install is
        // in, and it must not connect on its own.
        sleep(Duration::from_secs(1)).await;
        assert!(!worker.is_connected().await.expect("status is readable"));

        worker
            .set_credentials(Some(credentials_from_env_file()))
            .await
            .expect("credentials accepted");

        while !worker.is_connected().await.expect("status is readable") {
            sleep(Duration::from_millis(100)).await;
        }

        worker
            .set_voice_settings(true, false)
            .await
            .expect("voice settings accepted");
        sleep(Duration::from_secs(1)).await;
        assert!(worker.get_voice_settings().await.mute);

        worker
            .set_voice_settings(false, false)
            .await
            .expect("voice settings accepted");
        sleep(Duration::from_secs(1)).await;
        assert!(!worker.get_voice_settings().await.mute);

        worker.shutdown().await.expect("shuts down");
    }

    /// Exercises the event path rather than the command path: with the subscription in place,
    /// muting from Discord's own UI must reach the app without it polling anything.
    ///
    /// This is the empirical check that `VOICE_SETTINGS_UPDATE` actually fires, which the whole
    /// event-driven design depends on.
    #[tokio::test]
    #[ignore = "interactive: toggle mute in Discord within 30 seconds; run with --ignored"]
    async fn muting_from_discord_reaches_the_app_as_a_pushed_event() {
        let mut worker = DiscordWorker::new(Some(credentials_from_env_file())).await;
        worker.start().await.expect("start is accepted");

        while !worker.is_connected().await.expect("status is readable") {
            sleep(Duration::from_millis(100)).await;
        }

        let initial = worker.get_voice_settings().await;
        println!("Connected. Toggle mute in Discord now — waiting up to 30 s...");

        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while std::time::Instant::now() < deadline {
            if worker.get_voice_settings().await != initial {
                worker.shutdown().await.expect("shuts down");
                return;
            }
            sleep(Duration::from_millis(200)).await;
        }

        panic!("no VOICE_SETTINGS_UPDATE arrived within 30 s — the subscription is not working");
    }
}
