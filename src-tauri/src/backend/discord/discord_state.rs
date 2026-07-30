//! Connection state machine for Discord RPC.
//!
//! Nothing here runs on a timer while the connection is healthy. Voice state arrives as
//! `VOICE_SETTINGS_UPDATE` events pushed by Discord, and the only scheduled work is the
//! reconnect attempt, which backs off when Discord is not running.

use crate::credentials::{DISCORD_SCOPES, DiscordCredentials};
use crate::error::DiscordError;
use crate::ipc::{DiscordConnectionState, IpcClient, IpcEvent};
use crate::oauth;
use spawned_concurrency::tasks::{
    CallResponse, CastResponse, GenServer, GenServerHandle, send_after,
};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio::time::Duration;
use tracing::{debug, info, warn};

/// First delay after a failed connection attempt.
const RECONNECT_BACKOFF_MIN: Duration = Duration::from_millis(500);
/// Ceiling for the backoff. Discord being closed for an hour must not mean an hour of retries.
const RECONNECT_BACKOFF_MAX: Duration = Duration::from_secs(30);

pub type DiscordStateHandler = GenServerHandle<DiscordState>;

#[derive(Clone)]
pub enum InCallMessage {
    DiscordStatus,
    AccessToken,
    RefreshToken,
    VoiceSettings,
    Shutdown,
}

#[derive(Clone)]
pub enum InMessage {
    /// Attempts to bring the connection up. Rescheduled with backoff only while it fails.
    Connect,
    SetVoiceSetting(bool, bool),
    DisconnectChannel,
    /// Applies credentials entered at runtime, so connecting does not require a restart.
    SetCredentials(Box<Option<DiscordCredentials>>),
    /// Something the reader task observed.
    Ipc(IpcEvent),
}

#[derive(Clone, PartialEq)]
pub enum OutMessage {
    Done,
    DiscordStatus(DiscordConnectionState),
    AccessToken(Option<String>),
    RefreshToken(Option<String>),
    VoiceSettings(DiscordVoiceSettings),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct DiscordVoiceSettings {
    pub mute: bool,
    pub deafen: bool,
}

/// Something worth telling the controller about.
///
/// Emitted only on an actual change, so a consumer can treat every one of these as news and does
/// not need to compare against what it already knew.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DiscordWorkerEvent {
    VoiceSettingsChanged(DiscordVoiceSettings),
    ConnectionChanged { connected: bool },
}

/// `Clone` is required by the actor framework, not by this code: the state is moved through each
/// handler and handed back. Everything cloneable here is either cheap or an `Arc`.
#[derive(Clone)]
pub struct DiscordState {
    ipc_client: IpcClient,
    /// `None` until the user registers a Discord application. While it is `None` the state
    /// machine stays parked instead of retrying a connection it cannot possibly complete.
    credentials: Option<DiscordCredentials>,
    voice_settings: Arc<Mutex<DiscordVoiceSettings>>,
    /// Where changes are announced. The controller listens here instead of polling.
    observer: mpsc::UnboundedSender<DiscordWorkerEvent>,
    /// Last connection state announced, so `ConnectionChanged` really means changed.
    announced_connected: bool,
    reconnect_backoff: Duration,
    /// Guards against several reconnect timers piling up, which would defeat the backoff.
    reconnect_scheduled: bool,
    shutdown: bool,
}

impl DiscordState {
    pub async fn spawn(
        credentials: Option<DiscordCredentials>,
        voice_settings: Arc<Mutex<DiscordVoiceSettings>>,
        observer: mpsc::UnboundedSender<DiscordWorkerEvent>,
    ) -> DiscordStateHandler {
        let (events_tx, mut events_rx) = mpsc::unbounded_channel();

        let state = Self {
            ipc_client: IpcClient::new(events_tx),
            credentials,
            voice_settings,
            observer,
            announced_connected: false,
            reconnect_backoff: RECONNECT_BACKOFF_MIN,
            reconnect_scheduled: false,
            shutdown: false,
        };
        let handle = state.start();

        // Bridges the reader task's events into the actor's mailbox, so the actor only ever
        // reacts to messages and never polls anything.
        let mut forward_to = handle.clone();
        tokio::spawn(async move {
            while let Some(event) = events_rx.recv().await {
                if forward_to.cast(InMessage::Ipc(event)).await.is_err() {
                    break;
                }
            }
        });

        handle
    }

    /// Runs the connect → handshake → authorise → authenticate → subscribe sequence.
    ///
    /// Done in one pass rather than one step per timer tick: every step is awaited anyway, and a
    /// tick per step was the reason connecting used to take a second.
    async fn establish_connection(&mut self) -> Result<(), DiscordError> {
        let credentials = self
            .credentials
            .clone()
            .ok_or(DiscordError::ClientIdNotFound)?;

        self.ipc_client.connect().await?;
        self.ipc_client.handshake(&credentials.client_id).await?;

        let token = self.obtain_token(&credentials).await?;
        self.ipc_client.authenticate(&token).await?;

        // Subscribe first, then read the current value: doing it the other way round leaves a
        // window where a change is neither in the snapshot nor delivered as an event.
        self.ipc_client.subscribe_voice_settings().await?;

        let (mute, deafen) = self.ipc_client.get_voice_settings().await?;
        self.update_voice_settings(DiscordVoiceSettings { mute, deafen })
            .await;

        Ok(())
    }

    /// Produces a usable access token, reusing or refreshing what is stored before falling back
    /// to prompting the user with Discord's authorisation modal.
    async fn obtain_token(
        &mut self,
        credentials: &DiscordCredentials,
    ) -> Result<String, DiscordError> {
        if let Some(access_token) = credentials.access_token.clone() {
            return Ok(access_token);
        }

        if let Some(refresh_token) = credentials.refresh_token.clone() {
            match oauth::refresh_access_token(
                &credentials.client_id,
                &credentials.client_secret,
                &credentials.redirect_url,
                &refresh_token,
            )
            .await
            {
                Ok(tokens) => {
                    self.set_tokens(
                        Some(tokens.access_token.clone()),
                        Some(tokens.refresh_token),
                    );
                    return Ok(tokens.access_token);
                }
                Err(err) => {
                    warn!("Could not refresh the access token, asking the user again: {err}");
                    self.set_tokens(None, None);
                }
            }
        }

        let code = self.ipc_client.authorize(DISCORD_SCOPES).await?;

        let tokens = oauth::exchange_code(
            &credentials.client_id,
            &credentials.client_secret,
            &credentials.redirect_url,
            &code,
        )
        .await?;

        self.set_tokens(
            Some(tokens.access_token.clone()),
            Some(tokens.refresh_token),
        );
        Ok(tokens.access_token)
    }

    fn set_tokens(&mut self, access_token: Option<String>, refresh_token: Option<String>) {
        if let Some(credentials) = self.credentials.as_mut() {
            credentials.access_token = access_token;
            credentials.refresh_token = refresh_token;
        }
    }

    /// Records the voice state and announces it, but only if it actually moved.
    async fn update_voice_settings(&mut self, settings: DiscordVoiceSettings) {
        let mut current = self.voice_settings.lock().await;
        if *current == settings {
            return;
        }
        *current = settings;
        drop(current);

        let _ = self
            .observer
            .send(DiscordWorkerEvent::VoiceSettingsChanged(settings));
    }

    /// Announces a connection transition, once per transition.
    fn announce_connection(&mut self) {
        let connected = self.ipc_client.state == DiscordConnectionState::Authenticated;
        if connected == self.announced_connected {
            return;
        }
        self.announced_connected = connected;
        let _ = self
            .observer
            .send(DiscordWorkerEvent::ConnectionChanged { connected });
    }

    /// Schedules another attempt, doubling the delay up to the ceiling.
    fn schedule_reconnect(&mut self, handle: &GenServerHandle<Self>) {
        if self.shutdown || self.credentials.is_none() || self.reconnect_scheduled {
            return;
        }
        self.reconnect_scheduled = true;
        send_after(self.reconnect_backoff, handle.clone(), InMessage::Connect);
        self.reconnect_backoff = (self.reconnect_backoff * 2).min(RECONNECT_BACKOFF_MAX);
    }
}

impl GenServer for DiscordState {
    type CallMsg = InCallMessage;
    type CastMsg = InMessage;
    type OutMsg = OutMessage;
    type Error = DiscordError;

    async fn handle_cast(
        mut self,
        message: Self::CastMsg,
        handle: &GenServerHandle<Self>,
    ) -> CastResponse<Self> {
        if self.shutdown {
            return CastResponse::NoReply(self);
        }

        match message {
            InMessage::Connect => {
                self.reconnect_scheduled = false;

                if self.credentials.is_none() || self.ipc_client.is_connected() {
                    return CastResponse::NoReply(self);
                }

                if let Err(err) = self.establish_connection().await {
                    // Discord simply not running is the common case and not worth a warning.
                    match err {
                        DiscordError::PipeConnectionFailed => {
                            debug!("Discord is not running, will retry")
                        }
                        other => warn!("Could not connect to Discord: {other}"),
                    }
                    self.ipc_client.disconnect().await;
                    self.schedule_reconnect(handle);
                } else {
                    // Connected: reset the backoff so a later drop retries promptly.
                    self.reconnect_backoff = RECONNECT_BACKOFF_MIN;
                }
                self.announce_connection();
                CastResponse::NoReply(self)
            }

            InMessage::Ipc(IpcEvent::VoiceSettings { mute, deafen }) => {
                debug!("Voice settings pushed by Discord — mute: {mute} deafen: {deafen}");
                self.update_voice_settings(DiscordVoiceSettings { mute, deafen })
                    .await;
                CastResponse::NoReply(self)
            }

            InMessage::Ipc(IpcEvent::Disconnected) => {
                self.ipc_client.disconnect().await;
                self.announce_connection();
                self.schedule_reconnect(handle);
                CastResponse::NoReply(self)
            }

            InMessage::SetCredentials(credentials) => {
                let credentials = *credentials;
                if self.credentials == credentials {
                    return CastResponse::NoReply(self);
                }

                // Any live session belongs to the previous application.
                self.ipc_client.disconnect().await;
                self.announce_connection();
                self.credentials = credentials;
                self.reconnect_backoff = RECONNECT_BACKOFF_MIN;

                if self.credentials.is_some() {
                    info!("Discord credentials set, connecting");
                    self.schedule_reconnect(handle);
                } else {
                    info!("Discord credentials cleared, connection stopped");
                }
                CastResponse::NoReply(self)
            }

            InMessage::SetVoiceSetting(mute, deafen) => {
                if let Err(err) = self.ipc_client.set_voice_settings(mute, deafen).await {
                    warn!("Could not set voice settings: {err}");
                    self.ipc_client.disconnect().await;
                    self.announce_connection();
                    self.schedule_reconnect(handle);
                }
                CastResponse::NoReply(self)
            }

            InMessage::DisconnectChannel => {
                if let Err(err) = self.ipc_client.select_voice_channel(None).await {
                    warn!("Could not leave the voice channel: {err}");
                }
                CastResponse::NoReply(self)
            }
        }
    }

    async fn handle_call(
        mut self,
        message: Self::CallMsg,
        _handle: &GenServerHandle<Self>,
    ) -> CallResponse<Self> {
        match message {
            InCallMessage::DiscordStatus => {
                let state = self.ipc_client.state.clone();
                CallResponse::Reply(self, OutMessage::DiscordStatus(state))
            }
            InCallMessage::VoiceSettings => {
                let settings = *self.voice_settings.lock().await;
                CallResponse::Reply(self, OutMessage::VoiceSettings(settings))
            }
            InCallMessage::AccessToken => {
                let token = self
                    .credentials
                    .as_ref()
                    .and_then(|c| c.access_token.clone());
                CallResponse::Reply(self, OutMessage::AccessToken(token))
            }
            InCallMessage::RefreshToken => {
                let token = self
                    .credentials
                    .as_ref()
                    .and_then(|c| c.refresh_token.clone());
                CallResponse::Reply(self, OutMessage::RefreshToken(token))
            }
            InCallMessage::Shutdown => {
                self.shutdown = true;
                self.ipc_client.disconnect().await;
                CallResponse::Reply(self, OutMessage::Done)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> (DiscordState, mpsc::UnboundedReceiver<DiscordWorkerEvent>) {
        let (ipc_tx, _ipc_rx) = mpsc::unbounded_channel();
        let (observer, observed) = mpsc::unbounded_channel();
        (
            DiscordState {
                ipc_client: IpcClient::new(ipc_tx),
                credentials: None,
                voice_settings: Arc::new(Mutex::new(DiscordVoiceSettings::default())),
                observer,
                announced_connected: false,
                reconnect_backoff: RECONNECT_BACKOFF_MIN,
                reconnect_scheduled: false,
                shutdown: false,
            },
            observed,
        )
    }

    #[tokio::test]
    async fn a_voice_change_is_announced_once_and_repeats_are_swallowed() {
        // The coordinator treats every announcement as news and acts on it — pushing to the
        // device and emitting to the webview — so a duplicate here becomes wasted work there.
        let (mut state, mut observed) = state();
        let muted = DiscordVoiceSettings {
            mute: true,
            deafen: false,
        };

        state.update_voice_settings(muted).await;
        assert_eq!(
            observed.try_recv().expect("an announcement"),
            DiscordWorkerEvent::VoiceSettingsChanged(muted)
        );

        state.update_voice_settings(muted).await;
        assert!(
            observed.try_recv().is_err(),
            "the same value must not be announced twice"
        );

        assert_eq!(*state.voice_settings.lock().await, muted);
    }

    #[tokio::test]
    async fn a_connection_change_is_announced_once_per_transition() {
        let (mut state, mut observed) = state();

        // Still disconnected, so there is nothing to announce.
        state.announce_connection();
        assert!(observed.try_recv().is_err());

        state.ipc_client.state = DiscordConnectionState::Authenticated;
        state.announce_connection();
        assert_eq!(
            observed.try_recv().expect("an announcement"),
            DiscordWorkerEvent::ConnectionChanged { connected: true }
        );

        state.announce_connection();
        assert!(observed.try_recv().is_err());

        state.ipc_client.state = DiscordConnectionState::NotConnected;
        state.announce_connection();
        assert_eq!(
            observed.try_recv().expect("an announcement"),
            DiscordWorkerEvent::ConnectionChanged { connected: false }
        );
    }

    #[tokio::test]
    async fn only_full_authentication_counts_as_connected() {
        // A half-open connection must not light the indicator: the pipe being open says nothing
        // about whether the RPC session is usable.
        let (mut state, mut observed) = state();

        for intermediate in [
            DiscordConnectionState::Connected,
            DiscordConnectionState::HandshakeDone,
            DiscordConnectionState::Authorized,
        ] {
            state.ipc_client.state = intermediate;
            state.announce_connection();
            assert!(observed.try_recv().is_err());
        }
    }
}
