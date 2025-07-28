use crate::utils::*;

#[derive(Debug)]
pub struct PipeMessage {
    pub opcode: Opcode,
    pub length: u32,
    pub payload: Option<String>,
}

#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Opcode {
    Handshake = 0,
    Frame = 1,
    Close = 2,
    Ping = 3,
    Pong = 4,
    Error = 999,
}

impl Opcode {
    pub fn new(code: u32) -> Self {
        match code {
            0 => Opcode::Handshake,
            1 => Opcode::Frame,
            2 => Opcode::Close,
            3 => Opcode::Ping,
            4 => Opcode::Pong,
            _ => Opcode::Error,
        }
    }
}

impl PipeMessage {
    pub fn new(oc: Opcode, pl: &str) -> Self {
        Self {
            opcode: oc,
            length: pl.len() as u32,
            payload: Some(pl.to_string()),
        }
    }

    pub fn handshake(client_id: &str) -> Self {
        let pl: String = format!(r#"{{"v": 1,"client_id": "{}"}}"#, client_id);
        Self {
            opcode: Opcode::Handshake,
            length: pl.len() as u32,
            payload: Some(pl),
        }
    }

    pub fn to_buff(&self) -> Vec<u8> {
        let mut message: Vec<u8> = Vec::new();

        message.extend(&(self.opcode as u32).to_le_bytes());
        message.extend(&self.length.to_le_bytes());
        message.extend(self.payload.clone().unwrap().as_bytes());

        message
    }

    pub fn authorize(client_id: &str, scopes: &str) -> Self {
        let pl: String = format!(
            r#"{{"nonce": "{}", "cmd": "AUTHORIZE","args":{{ "client_id": "{}","scopes": "{}"}}}}"#,
            generate_nonce(36),
            client_id,
            scopes
        );
        Self {
            opcode: Opcode::Frame,
            length: pl.len() as u32,
            payload: Some(pl),
        }
    }

    pub fn authenticate(token: &str) -> Self {
        let pl: String = format!(
            r#"{{"nonce": "{}", "cmd": "AUTHENTICATE","args":{{ "access_token": "{}"}}}}"#,
            generate_nonce(36),
            token
        );
        Self {
            opcode: Opcode::Frame,
            length: pl.len() as u32,
            payload: Some(pl),
        }
    }

    pub fn get_voice_settings() -> Self {
        let pl: String = format!(
            r#"{{"nonce": "{}", "cmd": "GET_VOICE_SETTINGS"}}"#,
            generate_nonce(36)
        );
        Self {
            opcode: Opcode::Frame,
            length: pl.len() as u32,
            payload: Some(pl),
        }
    }

    pub fn set_voice_settings(muted: bool, deafed: bool) -> Self {
        let pl: String = format!(
            r#"{{"nonce": "{}", "cmd": "SET_VOICE_SETTINGS","args": {{"mute": {},"deaf":{}}}}}"#,
            generate_nonce(36),
            muted,
            deafed
        );
        Self {
            opcode: Opcode::Frame,
            length: pl.len() as u32,
            payload: Some(pl),
        }
    }

    pub fn select_voice_channel(channel_id: Option<String>) -> Self {
        let pl: String = format!(
            r#"{{"nonce": "{}", "cmd": "SELECT_VOICE_CHANNEL","args": {{"channel_id": {}}}}}"#,
            generate_nonce(36),
            match channel_id {
                Some(c) => format!(r#""{}""#, c),
                None => "null".to_string(),
            }
        );
        Self {
            opcode: Opcode::Frame,
            length: pl.len() as u32,
            payload: Some(pl),
        }
    }
}
