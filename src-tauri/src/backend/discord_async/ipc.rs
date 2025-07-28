use tokio::io::AsyncReadExt;
#[cfg(windows)]
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};
// use named_pipe::PipeClient;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Write};
#[cfg(unix)]
use std::{env::var, os::unix::net::UnixStream};

use crate::error::DiscordError;

use discord::pipemessage::PipeMessage;
pub struct IpcClient {
    #[cfg(unix)]
    pub pipe_client: Option<UnixStream>,

    #[cfg(windows)]
    pub pipe_client: Option<NamedPipeClient>,

    pub client_id: Option<String>,
}

impl IpcClient {
    pub fn new() -> Self {
        Self {
            pipe_client: None,
            client_id: None,
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
            } else {
                continue;
            }
        }

        return Err(DiscordError::PipeConnectionFailed);
    }

    pub async fn read_message(&mut self) -> Result<PipeMessage, DiscordError> {
        let mut buf = [0u8; 4];
        let received_opcode: u32;
        let received_length: u32;

        let Some(pipe_client) = &self.pipe_client else {
            return Err(DiscordError::PipeNotConnected);
        };

        // pipe_client.read_exact(&mut buf).await?;

        return Err(DiscordError::PipeNotConnected);

        // match &mut self.client_pipe {
        //     Some(cp) => {
        //         cp.read_exact(&mut buf)
        //             .map_err(|_| DiscordError::PipeErrorReading)?;
        //         received_opcode = u32::from_le_bytes(buf);

        //         cp.read_exact(&mut buf)
        //             .map_err(|_| DiscordError::PipeErrorReading)?;
        //         received_length = u32::from_le_bytes(buf);

        //         let mut response_data = vec![0u8; received_length as usize];
        //         cp.read_exact(&mut response_data)
        //             .map_err(|_| DiscordError::PipeErrorReading)?;
        //         let response_data_str = String::from_utf8_lossy(&response_data);

        //         return Ok(PipeMessage::new(
        //             Opcode::new(received_opcode),
        //             &response_data_str,
        //         ));
        //     }
        //     None => return Err(DiscordError::PipeNotConnected),
        // }
    }
}

// for running these test, discord is needed to be running on the background
#[cfg(test)]
mod tests {
    use super::IpcClient;

    #[tokio::test]
    async fn test_basic_connect() {
        let mut ipc_client = IpcClient::new();
        ipc_client.connect().unwrap();
        assert!(ipc_client.pipe_client.is_some());
    }
}
