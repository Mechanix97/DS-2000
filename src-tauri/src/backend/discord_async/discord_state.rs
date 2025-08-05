use crate::error::DiscordError;
use crate::ipc::{DiscordConnectionState, IpcClient};

use spawned_concurrency::tasks::{
    CallResponse, CastResponse, GenServer, GenServerHandle, send_after,
};
use std::time::Duration;
use tracing::debug;

const DISCORD_FETCH_INTERVAL: u64 = 100;

type DiscordStateHandler = GenServerHandle<DiscordState>;

#[derive(Clone)]
pub enum InCallMessage {
    Status,
}

#[derive(Clone)]
pub enum InMessage {
    Fetch,
    SetVoiceSetting(bool, bool),
}

#[derive(Clone, PartialEq)]
pub enum OutMessage {
    Done,
    Connected,
    NotConnected,
}

#[derive(Clone)]
pub struct DiscordState {
    fetch_interval_ms: u64,
    ipc_client: IpcClient,
    client_id: String,
    client_secret: String,
    redirect_url: String,
    code: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
}

impl DiscordState {
    pub fn new(client_id: String, client_secret: String, redirect_url: String) -> Self {
        Self {
            fetch_interval_ms: DISCORD_FETCH_INTERVAL,
            ipc_client: IpcClient::new(),
            client_id,
            client_secret,
            redirect_url,
            code: None,
            access_token: None,
            refresh_token: None,
        }
    }

    pub async fn spawn(
        client_id: String,
        client_secret: String,
        redirect_url: String,
    ) -> DiscordStateHandler {
        let state = Self::new(client_id, client_secret, redirect_url);
        state.start()
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
        match message {
            Self::CastMsg::Fetch => {
                match self.ipc_client.state {
                    DiscordConnectionState::NotConnected => {
                        debug!("Starting discord connection");
                        if let Err(err) = self.ipc_client.connect().await {
                            debug!("Error during discord connection start: {err}");
                            self.ipc_client.disconnect().await;
                        };
                    }
                    DiscordConnectionState::Connected => {
                        debug!("Doing Discord Handshake");

                        if let Err(err) = self.ipc_client.handshake(&self.client_id).await {
                            debug!("Error during discord handshake: {err}");
                            self.ipc_client.disconnect().await;
                        };
                    }
                    DiscordConnectionState::HandshakeDone => {
                        debug!("Doing Discord authorization");
                        match (&self.access_token, &self.refresh_token) {
                            (Some(access_token), Some(_)) => {
                                if let Err(err) = self.ipc_client.authenticate(access_token).await {
                                    debug!(
                                        "Error during discord authentication with stored token: {err}"
                                    );
                                    self.access_token = None;
                                }
                            }
                            (None, Some(refresh_token)) => {
                                match self
                                    .ipc_client
                                    .refresh_access_token(
                                        refresh_token,
                                        &self.client_secret,
                                        &self.redirect_url,
                                    )
                                    .await
                                {
                                    Ok((access_token, new_refresh_token)) => {
                                        self.access_token = Some(access_token.clone());
                                        self.refresh_token = Some(new_refresh_token.clone());

                                        if let Err(err) =
                                            self.ipc_client.authenticate(&access_token).await
                                        {
                                            debug!(
                                                "Error during discord authentication after refresh: {err}"
                                            );
                                            self.access_token = None;
                                            self.refresh_token = None;
                                        }
                                    }
                                    Err(err) => {
                                        debug!("Error refreshing access token: {err}");
                                        self.access_token = None;
                                        self.refresh_token = None;
                                    }
                                }
                            }
                            (Some(_), None) => {
                                debug!("shouldn't be here, restoring to previous state");
                                self.access_token = None;
                            }
                            (None, None) => match self.ipc_client.authorize().await {
                                Ok(code) => {
                                    self.code = Some(code);
                                }
                                Err(err) => {
                                    debug!("Error during discord authorization: {err}");
                                    self.ipc_client.disconnect().await;
                                }
                            },
                        }
                    }
                    DiscordConnectionState::Authorized => {
                        debug!("Doing Discord authentication");
                        let Some(code) = &self.code else {
                            debug!("Error: Code should be set");
                            self.ipc_client.disconnect().await;
                            return CastResponse::NoReply(self);
                        };
                        let token = match self
                            .ipc_client
                            .get_access_tokens(code, &self.client_secret, &self.redirect_url)
                            .await
                        {
                            Ok((access_token, refresh_token)) => {
                                self.access_token = Some(access_token.clone());
                                self.refresh_token = Some(refresh_token.clone());
                                access_token
                            }
                            Err(err) => {
                                debug!("Error getting access token {err}");
                                self.ipc_client.disconnect().await;
                                return CastResponse::NoReply(self);
                            }
                        };
                        if let Err(err) = self.ipc_client.authenticate(&token).await {
                            debug!("Error during discord authentication: {err}");
                            self.ipc_client.disconnect().await;
                        };
                    }
                    DiscordConnectionState::Authenticated => {
                        match self.ipc_client.get_voice_settings().await {
                            Ok((muted, deafen)) => {
                                debug!("muted: {muted}    deafen: {deafen}");
                            }
                            Err(err) => {
                                debug!("Error during discord authentication: {err}");
                                self.ipc_client.disconnect().await;
                            }
                        }
                    }
                }

                send_after(
                    Duration::from_millis(self.fetch_interval_ms),
                    handle.clone(),
                    Self::CastMsg::Fetch,
                );
                CastResponse::NoReply(self)
            }
            Self::CastMsg::SetVoiceSetting(mute, deafen) => {
                if let Err(err) = self.ipc_client.set_voice_settings(mute, deafen).await {
                    debug!("Error while setting voice settings {err}");
                    self.ipc_client.disconnect().await;
                }
                CastResponse::NoReply(self)
            }
        }
    }

    async fn handle_call(
        self,
        message: Self::CallMsg,
        _handle: &GenServerHandle<Self>,
    ) -> CallResponse<Self> {
        match message {
            Self::CallMsg::Status => {
                if self.ipc_client.state == DiscordConnectionState::Authenticated {
                    return CallResponse::Reply(self, OutMessage::Connected);
                }
                CallResponse::Reply(self, OutMessage::NotConnected)
            }
        }
    }
}

// for running these tests, discord should be running on the background
#[cfg(test)]
mod tests {
    use crate::discord_state::InCallMessage;
    use crate::discord_state::OutMessage;

    use super::DiscordState;
    use super::DiscordStateHandler;
    use super::InMessage;

    use std::fs::File;
    use std::io::{BufRead, BufReader};
    use tokio::time::{Duration, sleep};

    use std::path::PathBuf;

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

        let mut dw: DiscordStateHandler =
            DiscordState::spawn(client_id, client_secret, redirect_url).await;
        dw.cast(InMessage::Fetch).await.unwrap();

        while dw.call(InCallMessage::Status).await.unwrap() != OutMessage::Connected {}

        dw.cast(InMessage::SetVoiceSetting(true, true))
            .await
            .unwrap();
        sleep(Duration::from_secs(5)).await;
    }
}
