use crate::error::ControllerError;

use config::config::Config;
use config::credentials::{self, DISCORD_REDIRECT_URI, Secret};
use discord::credentials::DiscordCredentials;
use discord::discord_state::DiscordWorkerEvent;
use discord::discord_worker::DiscordWorker;
use serial::serial_state::SerialWorkerEvent;
use serial::serial_worker::SerialWorker;

use tokio::sync::mpsc;
use tokio::time::Duration;
use tracing::{debug, warn};

const DEFAULT_SERIAL_BAUDRATE: u32 = 115200;
const DEFAULT_SERIAL_TIMEOUT: Duration = Duration::from_millis(1000);

/// The receiving ends of the workers' event channels, handed to the coordinator.
///
/// Returned separately from the `Controller` because the coordinator needs to own them while the
/// controller itself lives behind a mutex shared with the Tauri command handlers.
pub struct WorkerEvents {
    pub discord: mpsc::UnboundedReceiver<DiscordWorkerEvent>,
    pub serial: mpsc::UnboundedReceiver<SerialWorkerEvent>,
}

pub struct Controller {
    pub discord_worker: DiscordWorker,
    pub serial_worker: SerialWorker,
    pub config: Config,
    pub coordinator_join_handle: Option<tokio::task::JoinHandle<()>>,
}

impl Controller {
    pub async fn new() -> (Self, WorkerEvents) {
        let mut config = Config::new();
        config.load().await;

        let (discord_tx, discord_rx) = mpsc::unbounded_channel();
        let (serial_tx, serial_rx) = mpsc::unbounded_channel();

        let discord_worker =
            DiscordWorker::new(load_discord_credentials(&config).await, discord_tx).await;
        let serial_worker =
            SerialWorker::new(DEFAULT_SERIAL_BAUDRATE, DEFAULT_SERIAL_TIMEOUT, serial_tx).await;

        (
            Controller {
                discord_worker,
                serial_worker,
                config,
                coordinator_join_handle: None,
            },
            WorkerEvents {
                discord: discord_rx,
                serial: serial_rx,
            },
        )
    }

    pub async fn start(&mut self) -> Result<(), ControllerError> {
        self.config.start().await;
        self.discord_worker.start().await?;

        let last_used_port = self.config.last_used_port().await;
        self.serial_worker.start(last_used_port).await?;
        Ok(())
    }

    pub async fn ds_set_voice_settings(
        &mut self,
        mute: bool,
        deaf: bool,
    ) -> Result<(), ControllerError> {
        self.discord_worker.set_voice_settings(mute, deaf).await?;
        Ok(())
    }

    /// Stores the Discord application the user registered and reconnects with it.
    pub async fn set_discord_credentials(
        &mut self,
        client_id: &str,
        client_secret: &str,
    ) -> Result<(), ControllerError> {
        self.config
            .set_discord_credentials(client_id, client_secret)
            .await?;

        let credentials = load_discord_credentials(&self.config).await;
        self.discord_worker.set_credentials(credentials).await?;
        Ok(())
    }

    /// Forgets the Discord application and stops the connection.
    pub async fn clear_discord_credentials(&mut self) -> Result<(), ControllerError> {
        self.config.clear_discord_credentials().await?;
        self.discord_worker.set_credentials(None).await?;
        Ok(())
    }

    pub async fn has_discord_credentials(&self) -> bool {
        self.config.has_discord_credentials().await
    }

    pub async fn shutdown(&mut self) -> Result<(), ControllerError> {
        debug!("Shutting down controller");
        if let Some(handle) = &mut self.coordinator_join_handle {
            handle.abort();
        }
        if let Err(err) = self.config.save().await {
            warn!("Could not save configuration during shutdown: {err}");
        }
        self.serial_worker.shutdown().await?;
        self.discord_worker.shutdown().await?;
        Ok(())
    }
}

/// Assembles credentials from the config file and the keyring.
///
/// Returns `None` unless both halves are present: a client id without its secret cannot complete
/// the OAuth exchange, so starting the worker would only produce a retry loop.
async fn load_discord_credentials(config: &Config) -> Option<DiscordCredentials> {
    let client_id = config.discord_client_id().await?;

    let client_secret = match credentials::read(Secret::ClientSecret) {
        Ok(Some(secret)) => secret,
        Ok(None) => return None,
        Err(err) => {
            warn!("Could not read the Discord client secret from the keyring: {err}");
            return None;
        }
    };

    let access_token = credentials::read(Secret::AccessToken).unwrap_or_default();
    let refresh_token = credentials::read(Secret::RefreshToken).unwrap_or_default();

    Some(
        DiscordCredentials::new(client_id, client_secret, DISCORD_REDIRECT_URI.to_owned())
            .with_tokens(access_token, refresh_token),
    )
}
