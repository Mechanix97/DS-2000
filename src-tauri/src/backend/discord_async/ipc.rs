use tokio::io::AsyncReadExt;

use tokio::io::AsyncWriteExt;
#[cfg(windows)]
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};

#[cfg(unix)]
use std::{env::var, os::unix::net::UnixStream};

use crate::error::DiscordError;

use discord::pipemessage::Opcode;
use discord::pipemessage::PipeMessage;

pub struct IpcClient {
    #[cfg(unix)]
    pub pipe_client: Option<UnixStream>,

    #[cfg(windows)]
    pub pipe_client: Option<NamedPipeClient>,

    pub client_id: Option<String>,
    pub connected: bool,
}

impl IpcClient {
    pub fn new() -> Self {
        Self {
            pipe_client: None,
            client_id: None,
            connected: false,
        }
    }

    #[cfg(windows)]
    pub fn connect(&mut self) -> Result<(), DiscordError> {
        let iter = 0..10;
        for i in iter {
            let pipe_name = format!(r"\\?\pipe\discord-ipc-{}", i);
            if let Ok(pipe) = ClientOptions::new().open(pipe_name) {
                self.pipe_client = Some(pipe);
                return Ok(());
            }
        }

        return Err(DiscordError::PipeConnectionFailed);
    }

    pub async fn read_message(&mut self) -> Result<PipeMessage, DiscordError> {
        let mut buf = [0u8; 4];
        let received_opcode: u32;
        let received_length: u32;

        let Some(pipe_client) = &mut self.pipe_client else {
            return Err(DiscordError::PipeNotConnected);
        };

        pipe_client.read_exact(&mut buf).await?;

        received_opcode = u32::from_le_bytes(buf);

        pipe_client.read_exact(&mut buf).await?;

        received_length = u32::from_le_bytes(buf);

        let response_data = vec![0u8; received_length as usize];

        let response_data_str = String::from_utf8_lossy(&response_data);

        return Ok(PipeMessage::new(
            Opcode::new(received_opcode),
            &response_data_str,
        ));
    }

    pub async fn handshake(&mut self, client_id: String) -> Result<(), DiscordError> {
        let Some(pipe_client) = &mut self.pipe_client else {
            return Err(DiscordError::PipeNotConnected);
        };
        //store client id
        self.client_id = Some(client_id.clone());

        pipe_client
            .write_all(&PipeMessage::handshake(&client_id).to_buff())
            .await?;

        if self.read_message().await?.opcode != Opcode::Frame {
            return Err(DiscordError::HandshakeFailed);
        }

        self.connected = true;
        return Ok(());
    }
}

// for running these tests, discord should be running on the background
#[cfg(test)]
mod tests {
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
                // Skip comments
                continue;
            };
            match line.split_once('=') {
                Some((key, value)) => unsafe { std::env::set_var(key, value) },
                None => continue,
            };
        }
    }

    #[tokio::test]
    async fn test_basic_connect() {
        load_env_file();
        let mut ipc_client: IpcClient = IpcClient::new();

        let client_id = std::env::var("DISCORD_CLIENT_ID").unwrap();

        ipc_client.connect().unwrap();
        assert!(ipc_client.pipe_client.is_some());

        ipc_client.handshake(client_id).await.unwrap();

        assert!(ipc_client.connected);
    }
}
