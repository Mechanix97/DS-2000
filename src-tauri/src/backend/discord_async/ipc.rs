use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
#[cfg(windows)]
use tokio::sync::Mutex;

use tokio::io::AsyncWriteExt;
#[cfg(windows)]
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};

#[cfg(unix)]
use {std::env::var, tokio::net::UnixStream};

use crate::error::DiscordError;
use crate::pipe_message::Opcode;
use crate::pipe_message::PipeMessage;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DiscordConnectionState {
    NotConnected,
    Connected,
    HandshakeDone,
    Authorized,
    Authenticated,
}

#[derive(Clone)]
pub struct IpcClient {
    #[cfg(unix)]
    pub pipe_client: Option<Arc<Mutex<UnixStream>>>,

    #[cfg(windows)]
    pub pipe_client: Option<Arc<Mutex<NamedPipeClient>>>,

    pub client_id: Option<String>,
    pub state: DiscordConnectionState,
}

impl IpcClient {
    pub fn new() -> Self {
        Self {
            pipe_client: None,
            client_id: None,
            state: DiscordConnectionState::NotConnected,
        }
    }

    #[cfg(windows)]
    pub async fn connect(&mut self) -> Result<(), DiscordError> {
        if self.pipe_client.is_some() || self.state != DiscordConnectionState::NotConnected {
            return Err(DiscordError::ClientAlreadyConnected);
        }
        let iter = 0..10;
        for i in iter {
            let pipe_name = format!(r"\\?\pipe\discord-ipc-{}", i);
            if let Ok(pipe) = ClientOptions::new().open(pipe_name) {
                self.pipe_client = Some(Arc::new(Mutex::new(pipe)));
                self.state = DiscordConnectionState::Connected;
                return Ok(());
            }
        }

        return Err(DiscordError::PipeConnectionFailed);
    }

    #[cfg(unix)]
    pub async fn connect(&mut self) -> Result<(), DiscordError> {
        if self.pipe_client.is_some() || self.state != DiscordConnectionState::NotConnected {
            return Err(DiscordError::ClientAlreadyConnected);
        }
        let mut sub_path = None;
        for key in ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"] {
            if let Ok(env_var) = var(key) {
                sub_path = Some(env_var);
            }
        }
        let sp = sub_path.ok_or(DiscordError::PipeConnectionFailed)?;
        for i in 0..10 {
            let pipe_name = format!("{}discord-ipc-{}", sp, i);
            if let Ok(pipe) = UnixStream::connect(&pipe_name).await {
                self.pipe_client = Some(Arc::new(Mutex::new(pipe)));
                self.state = DiscordConnectionState::Connected;
                return Ok(());
            }
        }

        Err(DiscordError::PipeConnectionFailed)
    }

    pub async fn disconnect(&mut self) {
        self.pipe_client = None;
        self.state = DiscordConnectionState::NotConnected;
    }

    pub async fn read_message(&mut self) -> Result<PipeMessage, DiscordError> {
        let Some(pipe_client) = &mut self.pipe_client else {
            return Err(DiscordError::PipeNotConnected);
        };

        let pipe_lock: tokio::sync::MutexGuard<'_, NamedPipeClient> = pipe_client.lock().await;

        read_message_from_lock(pipe_lock).await
    }

    pub async fn handshake(&mut self, client_id: &str) -> Result<(), DiscordError> {
        let Some(pipe_client) = &mut self.pipe_client else {
            return Err(DiscordError::PipeNotConnected);
        };
        //store client id
        self.client_id = Some(client_id.to_string());

        let mut pipe_lock = pipe_client.lock().await;

        pipe_lock
            .write_all(&PipeMessage::handshake(client_id).to_buff())
            .await?;

        if read_message_from_lock(pipe_lock).await?.opcode != Opcode::Frame {
            return Err(DiscordError::HandshakeFailed);
        }

        self.state = DiscordConnectionState::HandshakeDone;
        return Ok(());
    }

    pub async fn authorize(&mut self) -> Result<String, DiscordError> {
        let Some(pipe_client) = &mut self.pipe_client else {
            return Err(DiscordError::PipeNotConnected);
        };
        if self.state != DiscordConnectionState::HandshakeDone {
            return Err(DiscordError::HandshakeNotDone);
        }
        let Some(client_id) = &self.client_id else {
            return Err(DiscordError::ClientIdNotFound);
        };
        let mut pipe_lock = pipe_client.lock().await;

        pipe_lock
            .write_all(&PipeMessage::authorize(&client_id, "rpc").to_buff())
            .await?;

        //receive reply
        let m = read_message_from_lock(pipe_lock).await?;
        let payload = m.payload.ok_or(DiscordError::AuthorizationFailed)?;
        let parsed_json: serde_json::Value = serde_json::from_str(&payload)?;

        if !(parsed_json["evt"].is_null()) {
            return Err(DiscordError::AuthorizationFailed);
        }
        self.state = DiscordConnectionState::Authorized;
        Ok(parsed_json["data"]["code"]
            .to_string()
            .trim_matches('"')
            .to_owned())
    }

    pub async fn get_access_tokens(
        &mut self,
        code: &str,
        client_secret: &str,
        redirect_url: &str,
    ) -> Result<(String, String), DiscordError> {
        let Some(client_id) = &self.client_id else {
            return Err(DiscordError::ClientIdNotFound);
        };

        let api_endpoint = "https://discord.com/api/v10/oauth2/token";
        let cs = client_secret.to_string();
        let ac = "authorization_code".to_string();
        let c = code.to_string();
        let ru = redirect_url.to_string();
        let mut data = HashMap::new();

        data.insert("client_id", client_id);
        data.insert("client_secret", &cs);
        data.insert("grant_type", &ac);
        data.insert("code", &c);
        data.insert("redirect_uri", &ru);

        let ds = reqwest::Client::new();
        let res = ds
            .post(api_endpoint)
            .form(&data)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .send()
            .await?;
        let body = res.text().await?;

        let response: Value = serde_json::from_str(&body)?;

        Ok((
            response["access_token"]
                .to_string()
                .trim_matches('"')
                .to_string(),
            response["refresh_token"]
                .to_string()
                .trim_matches('"')
                .to_string(),
        ))
    }

    pub async fn refresh_access_token(
        &mut self,
        refresh_token: &str,
        client_secret: &str,
        redirect_url: &str,
    ) -> Result<(String, String), DiscordError> {
        let Some(client_id) = &self.client_id else {
            return Err(DiscordError::ClientIdNotFound);
        };

        let api_endpoint = "https://discord.com/api/v10/oauth2/token";
        let cs = client_secret.to_string();
        let gt = "refresh_token".to_string();
        let rt = refresh_token.to_string();
        let ru = redirect_url.to_string();
        let mut data = HashMap::new();

        data.insert("client_id", client_id);
        data.insert("client_secret", &cs);
        data.insert("grant_type", &gt);
        data.insert("refresh_token", &rt);
        data.insert("redirect_uri", &ru);

        let ds = reqwest::Client::new();
        let res = ds
            .post(api_endpoint)
            .form(&data)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .send()
            .await?;
        let body = res.text().await?;

        let response: Value = serde_json::from_str(&body)?;

        Ok((
            response["access_token"]
                .to_string()
                .trim_matches('"')
                .to_string(),
            response["refresh_token"]
                .to_string()
                .trim_matches('"')
                .to_string(),
        ))
    }

    pub async fn authenticate(&mut self, token: &str) -> Result<(), DiscordError> {
        let Some(pipe_client) = &mut self.pipe_client else {
            return Err(DiscordError::PipeNotConnected);
        };
        if self.state != DiscordConnectionState::Authorized
            && self.state != DiscordConnectionState::HandshakeDone
        {
            return Err(DiscordError::AuthorizationFailed);
        }
        let mut pipe_lock = pipe_client.lock().await;

        pipe_lock
            .write_all(&PipeMessage::authenticate(token).to_buff())
            .await?;

        let response = read_message_from_lock(pipe_lock).await?;
        let payload = response.payload.ok_or(DiscordError::AuthenticationFailed)?;
        let parsed_json: serde_json::Value = serde_json::from_str(&payload)?;
        if !(parsed_json["evt"].is_null()) {
            return Err(DiscordError::AuthenticationFailed);
        }

        self.state = DiscordConnectionState::Authenticated;
        Ok(())
    }

    pub async fn get_voice_settings(&mut self) -> Result<(bool, bool), DiscordError> {
        let Some(pipe_client) = &mut self.pipe_client else {
            return Err(DiscordError::PipeNotConnected);
        };

        if self.state != DiscordConnectionState::Authenticated {
            return Err(DiscordError::ClientNotConnected);
        }

        let mut pipe_lock = pipe_client.lock().await;

        pipe_lock
            .write_all(&PipeMessage::get_voice_settings().to_buff())
            .await?;

        let response = read_message_from_lock(pipe_lock).await?;
        let payload = response.payload.ok_or(DiscordError::AuthenticationFailed)?;
        let parsed_json: serde_json::Value = serde_json::from_str(&payload)?;
        if !(parsed_json["evt"].is_null()) {
            return Err(DiscordError::AuthenticationFailed);
        }

        if parsed_json["data"]["mute"].is_null() || parsed_json["data"]["deaf"].is_null() {
            return Err(DiscordError::NoDataFound);
        }

        let muted = parsed_json["data"]["mute"].as_bool().unwrap();
        let deafen = parsed_json["data"]["deaf"].as_bool().unwrap();
        Ok((muted, deafen))
    }

    pub async fn set_voice_settings(
        &mut self,
        muted: bool,
        deafed: bool,
    ) -> Result<(), DiscordError> {
        let Some(pipe_client) = &mut self.pipe_client else {
            return Err(DiscordError::PipeNotConnected);
        };
        if self.state != DiscordConnectionState::Authenticated {
            return Err(DiscordError::ClientNotConnected);
        }

        let mut pipe_lock = pipe_client.lock().await;

        pipe_lock
            .write_all(&PipeMessage::set_voice_settings(muted, deafed).to_buff())
            .await?;

        let response = read_message_from_lock(pipe_lock).await?;
        let payload = response.payload.ok_or(DiscordError::AuthenticationFailed)?;
        let parsed_json: serde_json::Value = serde_json::from_str(&payload)?;
        if !(parsed_json["evt"].is_null()) {
            return Err(DiscordError::AuthenticationFailed);
        }

        Ok(())
    }

    pub async fn select_voice_channel(
        &mut self,
        channel_id: Option<String>,
    ) -> Result<(), DiscordError> {
        let Some(pipe_client) = &mut self.pipe_client else {
            return Err(DiscordError::PipeNotConnected);
        };
        if self.state != DiscordConnectionState::Authenticated {
            return Err(DiscordError::ClientNotConnected);
        }

        let mut pipe_lock = pipe_client.lock().await;

        pipe_lock
            .write_all(&PipeMessage::select_voice_channel(channel_id).to_buff())
            .await?;

        let response = read_message_from_lock(pipe_lock).await?;
        let payload = response.payload.ok_or(DiscordError::AuthenticationFailed)?;
        let parsed_json: serde_json::Value = serde_json::from_str(&payload)?;
        if !(parsed_json["evt"].is_null()) {
            return Err(DiscordError::AuthenticationFailed);
        }

        Ok(())
    }
}
pub async fn read_message_from_lock(
    mut pipe_lock: tokio::sync::MutexGuard<'_, NamedPipeClient>,
) -> Result<PipeMessage, DiscordError> {
    let mut buf = [0u8; 4];
    let received_opcode: u32;
    let received_length: u32;

    pipe_lock.read_exact(&mut buf).await?;

    received_opcode = u32::from_le_bytes(buf);

    pipe_lock.read_exact(&mut buf).await?;

    received_length = u32::from_le_bytes(buf);

    let mut response_data = vec![0u8; received_length as usize];
    pipe_lock.read_exact(&mut response_data).await?;

    let response_data_str = String::from_utf8_lossy(&response_data).into_owned();

    return Ok(PipeMessage::new(
        Opcode::new(received_opcode),
        &response_data_str,
    ));
}

// for running these tests, discord should be running on the background
#[cfg(test)]
mod tests {
    use super::DiscordConnectionState;
    use super::IpcClient;

    use std::fs::File;
    use std::io::{BufRead, BufReader};

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
    async fn test_discord_connection() {
        load_env_file();
        let mut ipc_client: IpcClient = IpcClient::new();

        let client_id = std::env::var("DISCORD_CLIENT_ID").unwrap();
        let client_secret = std::env::var("DISCORD_SECRET_KEY").unwrap();
        let redirect_uri = "https://www.mechardo3d.xyz/";

        ipc_client.connect().await.unwrap();
        assert!(ipc_client.pipe_client.is_some());
        assert_eq!(ipc_client.state, DiscordConnectionState::Connected);

        ipc_client.handshake(&client_id).await.unwrap();

        assert_eq!(ipc_client.state, DiscordConnectionState::HandshakeDone);

        let code = ipc_client.authorize().await.unwrap();

        assert_eq!(ipc_client.state, DiscordConnectionState::Authorized);

        let (token, _rt) = ipc_client
            .get_access_tokens(&code, &client_secret, redirect_uri)
            .await
            .unwrap();

        ipc_client.authenticate(&token).await.unwrap();

        assert_eq!(ipc_client.state, DiscordConnectionState::Authenticated);

        ipc_client.set_voice_settings(true, false).await.unwrap();

        let (muted, deafen) = ipc_client.get_voice_settings().await.unwrap();

        assert!(muted);
        assert!(!deafen);

        ipc_client.set_voice_settings(true, true).await.unwrap();

        let (muted, deafen) = ipc_client.get_voice_settings().await.unwrap();

        assert!(muted);
        assert!(deafen);
    }
}
