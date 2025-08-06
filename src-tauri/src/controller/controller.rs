use crate::error::ControllerError;

use config::config::Config;
use discord::discord_worker::DiscordWorker;
use serial::serial_worker::SerialWorker;

use tokio::time::Duration;
use tracing::debug;

const DEFAULT_SERIAL_BAUDRATE: u32 = 115200;
const DEFAULT_SERIAL_TIMEOUT: u64 = 1000; //millis

pub struct Controller {
    pub discord_worker: DiscordWorker,
    pub serial_worker: SerialWorker,
    pub config: Config,
    pub backgroung_join_handle: Option<tokio::task::JoinHandle<Result<(), ControllerError>>>,
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

        let serial_worker = SerialWorker::new(
            DEFAULT_SERIAL_BAUDRATE,
            Duration::from_millis(DEFAULT_SERIAL_TIMEOUT),
        )
        .await;

        Controller {
            discord_worker,
            serial_worker,
            config,
            backgroung_join_handle: None,
        }
    }

    pub async fn start(&mut self) -> Result<(), ControllerError> {
        self.config.start().await;

        self.discord_worker.start().await?;

        let last_used_port = self.config.get_last_used_port().await;
        self.serial_worker.start(last_used_port).await?;
        Ok(())
    }

    pub async fn ds_set_voice_settings(&mut self, mute: bool, deaf: bool) {
        self.discord_worker
            .set_voice_settings(mute, deaf)
            .await
            .unwrap();
    }

    pub async fn shutdown(&mut self) -> Result<(), ControllerError> {
        debug!("shuting down controller");
        if let Some(jh) = &mut self.backgroung_join_handle {
            jh.abort();
        }
        self.config.save().await;
        self.serial_worker.shutdown().await?;
        self.discord_worker.shutdown().await?;
        Ok(())
    }
}
