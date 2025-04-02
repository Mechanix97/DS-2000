use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Write};

use crate::discord::error::*;
use crate::discord::pipemessage::*;

pub struct IPCClient {
    client_pipe: Option<u32>,
    client_id: Option<String>,
}

impl IPCClient {
    pub fn new() -> Self {
        Self {
            client_pipe: None,
            client_id: None,
        }
    }

    pub fn connect(&mut self) -> Result<(), DiscordError> {
        Ok(())
    }

    pub fn read_message(&mut self) -> Result<(), DiscordError> {
        Ok(())
    }

    pub fn handshake(&mut self, client_id: String) -> Result<(), DiscordError> {
        Ok(())
    }

    pub fn authorize(&mut self) -> Result<String, DiscordError> {
        Ok("HOLA".to_string())
    }

    pub fn authenticate(&mut self, token: &str) -> Result<(), DiscordError> {
        Ok(())
    }

    pub fn get_access_token(
        &mut self,
        code: &str,
        client_secret: &str,
        redirect_uri: &str,
    ) -> String {
        "HOLA".to_string()
    }

    pub fn get_voice_settings(&mut self) -> Result<(bool, bool), DiscordError> {
        Ok((false,false))
    }

    pub fn set_voice_settings(&mut self, muted: bool, deafed: bool) -> Result<(), DiscordError> {
       Ok(())
    }

    pub fn select_voice_channel(&mut self, channel_id: Option<String>) -> Result<(), DiscordError> {
       Ok(())
    }
}
