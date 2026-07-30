use crate::credentials::DiscordCredentials;
use crate::discord_state::{
    DiscordState, DiscordStateHandler, DiscordVoiceSettings, DiscordWorkerEvent, InCallMessage,
    InMessage, OutMessage,
};
use crate::error::DiscordError;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{Mutex, mpsc};

pub struct DiscordWorker {
    discord_handler: DiscordStateHandler,
    /// Shared with the state machine so reading the current voice state does not need a
    /// round-trip through the actor mailbox.
    voice_settings: Arc<Mutex<DiscordVoiceSettings>>,
    /// Also shared, and for a sharper reason: a connection attempt waits on the user clicking
    /// Discord's authorisation modal, and asking the actor during that window would block until
    /// the call timed out.
    connected: Arc<AtomicBool>,
}

impl DiscordWorker {
    /// Creates the worker. `credentials` is `None` until the user registers a Discord
    /// application; the worker stays idle in that case and starts on [`Self::set_credentials`].
    ///
    /// Changes are announced on `observer` as they happen; nothing has to poll this worker.
    pub async fn new(
        credentials: Option<DiscordCredentials>,
        observer: mpsc::UnboundedSender<DiscordWorkerEvent>,
    ) -> Self {
        let voice_settings = Arc::new(Mutex::new(DiscordVoiceSettings::default()));
        let connected = Arc::new(AtomicBool::new(false));
        let discord_handler = DiscordState::spawn(
            credentials,
            voice_settings.clone(),
            observer,
            connected.clone(),
        )
        .await;

        Self {
            discord_handler,
            voice_settings,
            connected,
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

    /// Whether an authenticated RPC session is open.
    ///
    /// Reads a shared flag rather than messaging the actor, so it answers even while a connection
    /// attempt is parked on the user clicking Discord's authorisation modal.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
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
        let (observer, _observed) = mpsc::unbounded_channel();
        let mut worker = DiscordWorker::new(None, observer).await;

        worker.start().await.expect("start is accepted");

        assert!(!worker.is_connected());
        assert!(worker.get_access_token().await.expect("readable").is_none());
    }

    #[tokio::test]
    async fn an_idle_worker_never_reports_a_connection_on_its_own() {
        // Guards the "stay parked without credentials" behaviour: a regression here would mean
        // burning CPU retrying a connection that cannot succeed.
        let (observer, _observed) = mpsc::unbounded_channel();
        let mut worker = DiscordWorker::new(None, observer).await;
        worker.start().await.expect("start is accepted");

        sleep(Duration::from_millis(300)).await;

        assert!(!worker.is_connected());
        assert_eq!(
            worker.get_voice_settings().await,
            DiscordVoiceSettings::default()
        );
    }

    /// Waits for an authenticated session, giving up rather than hanging forever.
    ///
    /// The first run needs a human to accept Discord's authorisation modal, so the window is
    /// generous — but a test that never returns tells you nothing.
    async fn wait_until_connected(worker: &DiscordWorker) {
        let deadline = std::time::Instant::now() + Duration::from_secs(150);
        while std::time::Instant::now() < deadline {
            if worker.is_connected() {
                return;
            }
            sleep(Duration::from_millis(100)).await;
        }
        panic!("no authenticated session after 150 s — was the authorisation modal accepted?");
    }

    /// Installs a log subscriber so `RUST_LOG` works in these tests.
    ///
    /// Without it the interactive tests are silent, and a connection stuck waiting on the
    /// authorisation modal is indistinguishable from one failing for any other reason.
    fn init_logging() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "discord=debug".into()),
            )
            .with_test_writer()
            .try_init();
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
        init_logging();
        let (observer, _observed) = mpsc::unbounded_channel();
        let mut worker = DiscordWorker::new(None, observer).await;
        worker.start().await.expect("start is accepted");

        // Nothing should happen until credentials arrive: this is the state a fresh install is
        // in, and it must not connect on its own.
        sleep(Duration::from_secs(1)).await;
        assert!(!worker.is_connected());

        worker
            .set_credentials(Some(credentials_from_env_file()))
            .await
            .expect("credentials accepted");

        wait_until_connected(&worker).await;

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
        init_logging();
        let (observer, _observed) = mpsc::unbounded_channel();
        let mut worker = DiscordWorker::new(Some(credentials_from_env_file()), observer).await;
        worker.start().await.expect("start is accepted");

        wait_until_connected(&worker).await;

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
