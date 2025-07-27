#[cfg(windows)]
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};
// use named_pipe::PipeClient;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Write};
#[cfg(unix)]
use std::{env::var, os::unix::net::UnixStream};

use crate::backend::discord::error::*;

pub struct IpcClient {
    #[cfg(unix)]
    pub client_pipe: Option<UnixStream>,

    #[cfg(windows)]
    pub client_pipe: Option<NamedPipeClient>,

    pub client_id: Option<String>,
}

impl IpcClient {
    pub fn new() -> Self {
        Self {
            client_pipe: None,
            client_id: None,
        }
    }

    #[cfg(windows)]
    pub fn connect(&mut self) -> Result<(), DiscordError> {
        let iter = 0..10;
        for i in iter {
            let pipe_name = format!(r"\\?\pipe\discord-ipc-{}", i);
            if let Ok(pipe) = ClientOptions::new().open(pipe_name) {
                self.client_pipe = Some(pipe);
                return Ok(());
            } else {
                continue;
            }
        }

        return Err(DiscordError::PipeConnectionFailed);
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
        assert!(ipc_client.client_pipe.is_some());
    }
}
