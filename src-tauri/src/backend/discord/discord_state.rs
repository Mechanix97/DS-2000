use crate::credentials::DiscordCredentials;
use crate::error::DiscordError;
use crate::ipc::{DiscordConnectionState, IpcClient};
use spawned_concurrency::tasks::{
    CallResponse, CastResponse, GenServer, GenServerHandle, send_after,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration;
use tracing::{debug, info, warn};

const DISCORD_FETCH_INTERVAL: u64 = 250;

pub type DiscordStateHandler = GenServerHandle<DiscordState>;

#[derive(Clone)]
pub enum InCallMessage {
    DiscordStatus,
    AccessToken,
    RefreshToken,
    Shutdown,
}

#[derive(Clone)]
pub enum InMessage {
    Fetch,
    SetVoiceSetting(bool, bool),
    DisconnectChannel,
    /// Applies credentials entered at runtime, so connecting does not require a restart.
    SetCredentials(Box<Option<DiscordCredentials>>),
}

#[derive(Clone, PartialEq)]
pub enum OutMessage {
    Done,
    DiscordStatus(DiscordConnectionState),
    AccessToken(Option<String>),
    RefreshToken(Option<String>),
}

#[derive(Clone, PartialEq, Eq)]
pub struct DiscordVoiceSettings {
    pub mute: bool,
    pub deafen: bool,
}

#[derive(Clone)]
pub struct DiscordState {
    pub fetch_interval_ms: u64,
    pub ipc_client: IpcClient,
    /// `None` until the user registers a Discord application. While it is `None` the state
    /// machine stays parked instead of retrying a connection it cannot possibly complete.
    pub credentials: Option<DiscordCredentials>,
    pub code: Option<String>,
    pub voice_setting: Arc<Mutex<DiscordVoiceSettings>>,
    pub shutdown: bool,
}

impl DiscordState {
    pub fn new(
        credentials: Option<DiscordCredentials>,
        voice_setting: Arc<Mutex<DiscordVoiceSettings>>,
    ) -> Self {
        Self {
            fetch_interval_ms: DISCORD_FETCH_INTERVAL,
            ipc_client: IpcClient::new(),
            credentials,
            code: None,
            voice_setting,
            shutdown: false,
        }
    }

    pub async fn spawn(
        credentials: Option<DiscordCredentials>,
        voice_setting: Arc<Mutex<DiscordVoiceSettings>>,
    ) -> DiscordStateHandler {
        Self::new(credentials, voice_setting).start()
    }

    /// Advances the connection state machine by one step.
    ///
    /// Split out of `handle_cast` so the message handler stays readable and so the "no
    /// credentials" short-circuit has one obvious home.
    async fn advance(&mut self) {
        let Some(credentials) = self.credentials.clone() else {
            return;
        };

        match self.ipc_client.state {
            DiscordConnectionState::NotConnected => {
                debug!("Starting Discord connection");
                if let Err(err) = self.ipc_client.connect().await {
                    debug!("Discord is not running or the pipe is unavailable: {err}");
                    self.ipc_client.disconnect().await;
                }
            }
            DiscordConnectionState::Connected => {
                debug!("Performing Discord handshake");
                if let Err(err) = self.ipc_client.handshake(&credentials.client_id).await {
                    warn!("Discord handshake failed: {err}");
                    self.ipc_client.disconnect().await;
                }
            }
            DiscordConnectionState::HandshakeDone => {
                self.authorize_or_authenticate(&credentials).await;
            }
            DiscordConnectionState::Authorized => {
                self.exchange_code(&credentials).await;
            }
            DiscordConnectionState::Authenticated => {
                match self.ipc_client.get_voice_settings().await {
                    Ok((mute, deafen)) => {
                        *self.voice_setting.lock().await = DiscordVoiceSettings { mute, deafen };
                    }
                    Err(err) => {
                        warn!("Could not read voice settings, dropping the connection: {err}");
                        self.ipc_client.disconnect().await;
                    }
                }
            }
        }
    }

    /// Reuses stored tokens when possible, refreshes them when only the refresh token survives,
    /// and falls back to prompting the user with the authorisation modal.
    async fn authorize_or_authenticate(&mut self, credentials: &DiscordCredentials) {
        match (
            self.credentials
                .as_ref()
                .and_then(|c| c.access_token.clone()),
            self.credentials
                .as_ref()
                .and_then(|c| c.refresh_token.clone()),
        ) {
            (Some(access_token), _) => {
                if let Err(err) = self.ipc_client.authenticate(&access_token).await {
                    debug!("Stored access token rejected, will try to refresh: {err}");
                    self.set_access_token(None);
                }
            }
            (None, Some(refresh_token)) => {
                match self
                    .ipc_client
                    .refresh_access_token(
                        &refresh_token,
                        &credentials.client_secret,
                        &credentials.redirect_url,
                    )
                    .await
                {
                    Ok((access_token, new_refresh_token)) => {
                        self.set_tokens(Some(access_token.clone()), Some(new_refresh_token));
                        if let Err(err) = self.ipc_client.authenticate(&access_token).await {
                            warn!("Authentication failed after refreshing the token: {err}");
                            self.set_tokens(None, None);
                        }
                    }
                    Err(err) => {
                        warn!("Could not refresh the access token, reauthorisation needed: {err}");
                        self.set_tokens(None, None);
                    }
                }
            }
            (None, None) => match self.ipc_client.authorize().await {
                Ok(code) => self.code = Some(code),
                Err(err) => {
                    warn!("Discord authorisation was refused or dismissed: {err}");
                    self.ipc_client.disconnect().await;
                }
            },
        }
    }

    async fn exchange_code(&mut self, credentials: &DiscordCredentials) {
        let Some(code) = self.code.clone() else {
            warn!("Reached the authorized state without an authorisation code");
            self.ipc_client.disconnect().await;
            return;
        };

        let token = match self
            .ipc_client
            .get_access_tokens(&code, &credentials.client_secret, &credentials.redirect_url)
            .await
        {
            Ok((access_token, refresh_token)) => {
                self.set_tokens(Some(access_token.clone()), Some(refresh_token));
                access_token
            }
            Err(err) => {
                warn!("Could not exchange the authorisation code for a token: {err}");
                self.ipc_client.disconnect().await;
                return;
            }
        };

        if let Err(err) = self.ipc_client.authenticate(&token).await {
            warn!("Authentication with the fresh token failed: {err}");
            self.ipc_client.disconnect().await;
        }
    }

    fn set_access_token(&mut self, access_token: Option<String>) {
        if let Some(credentials) = self.credentials.as_mut() {
            credentials.access_token = access_token;
        }
    }

    fn set_tokens(&mut self, access_token: Option<String>, refresh_token: Option<String>) {
        if let Some(credentials) = self.credentials.as_mut() {
            credentials.access_token = access_token;
            credentials.refresh_token = refresh_token;
        }
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
            Self::CastMsg::Fetch => {
                if self.credentials.is_none() {
                    // Parked: no application registered yet. `SetCredentials` restarts the loop,
                    // so there is nothing to reschedule and nothing to burn CPU on.
                    return CastResponse::NoReply(self);
                }

                self.advance().await;

                send_after(
                    Duration::from_millis(self.fetch_interval_ms),
                    handle.clone(),
                    Self::CastMsg::Fetch,
                );
                CastResponse::NoReply(self)
            }
            Self::CastMsg::SetCredentials(credentials) => {
                let credentials = *credentials;
                let had_credentials = self.credentials.is_some();
                let changed = self.credentials != credentials;

                if changed {
                    // Any live session belongs to the previous application.
                    self.ipc_client.disconnect().await;
                    self.code = None;
                    self.credentials = credentials;
                }

                if self.credentials.is_some() {
                    info!("Discord credentials set, starting connection");
                    if !had_credentials || changed {
                        send_after(
                            Duration::from_millis(0),
                            handle.clone(),
                            Self::CastMsg::Fetch,
                        );
                    }
                } else {
                    info!("Discord credentials cleared, connection stopped");
                }
                CastResponse::NoReply(self)
            }
            Self::CastMsg::SetVoiceSetting(mute, deafen) => {
                if let Err(err) = self.ipc_client.set_voice_settings(mute, deafen).await {
                    warn!("Could not set voice settings: {err}");
                    self.ipc_client.disconnect().await;
                }
                CastResponse::NoReply(self)
            }
            Self::CastMsg::DisconnectChannel => {
                if let Err(err) = self.ipc_client.select_voice_channel(None).await {
                    warn!("Could not leave the voice channel: {err}");
                    self.ipc_client.disconnect().await;
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
            Self::CallMsg::DiscordStatus => {
                let state = self.ipc_client.state.clone();
                CallResponse::Reply(self, OutMessage::DiscordStatus(state))
            }
            Self::CallMsg::AccessToken => {
                let token = self
                    .credentials
                    .as_ref()
                    .and_then(|c| c.access_token.clone());
                CallResponse::Reply(self, OutMessage::AccessToken(token))
            }
            Self::CallMsg::RefreshToken => {
                let token = self
                    .credentials
                    .as_ref()
                    .and_then(|c| c.refresh_token.clone());
                CallResponse::Reply(self, OutMessage::RefreshToken(token))
            }
            Self::CallMsg::Shutdown => {
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

    fn voice_settings() -> Arc<Mutex<DiscordVoiceSettings>> {
        Arc::new(Mutex::new(DiscordVoiceSettings {
            mute: false,
            deafen: false,
        }))
    }

    #[tokio::test]
    async fn without_credentials_the_state_machine_does_not_touch_the_pipe() {
        let mut state = DiscordState::new(None, voice_settings());

        state.advance().await;

        assert_eq!(state.ipc_client.state, DiscordConnectionState::NotConnected);
        assert!(state.code.is_none());
    }

    #[tokio::test]
    async fn tokens_are_readable_back_from_the_credentials() {
        let credentials = DiscordCredentials::new(
            "id".to_owned(),
            "secret".to_owned(),
            "http://localhost/".to_owned(),
        )
        .with_tokens(Some("access".to_owned()), Some("refresh".to_owned()));

        let mut state = DiscordState::new(Some(credentials), voice_settings());

        assert_eq!(
            state
                .credentials
                .as_ref()
                .and_then(|c| c.access_token.clone()),
            Some("access".to_owned())
        );

        state.set_tokens(None, None);

        assert!(
            state
                .credentials
                .as_ref()
                .and_then(|c| c.access_token.clone())
                .is_none()
        );
    }
}
