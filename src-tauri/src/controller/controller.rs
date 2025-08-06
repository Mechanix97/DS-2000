use crate::error::ControllerError;

use config::config::Config;
use discord::discord_worker::DiscordWorker;
use serial::serial_worker::SerialWorker;

pub struct Controller {
    pub discord_worker: DiscordWorker,
    pub serial_worker: SerialWorker,
    pub config: Config,
}

impl Controller {
    pub async fn new() -> Self {
        let mut config = Config::new();
        config.load().await;

        let discord_worker = DiscordWorker::new(
            config.get_discord_client_id().await,
            config.get_discord_secret_key().await,
            config.get_redirect_url().await,
            config.get_discord_access_token().await,
            config.get_discord_refresh_token().await,
        )
        .await;

        Controller {
            discord_worker: discord_worker,
            serial_worker: SerialWorker::new(),
            config: config,
        }
    }

    pub async fn start(&mut self) -> Result<(), ControllerError> {
        self.config.start().await;

        self.discord_worker.start().await?;

        // let last_used_port = self.config.last_port_connected.clone();
        // self.serial_worker.start(last_used_port).await?;
        Ok(())
    }

    pub async fn ds_set_voice_settings(&mut self, mute: bool, deaf: bool) {
        self.discord_worker
            .set_voice_settings(mute, deaf)
            .await
            .unwrap();
    }
}
