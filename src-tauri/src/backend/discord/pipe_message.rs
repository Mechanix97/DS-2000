//! Wire format for Discord's local RPC protocol.
//!
//! A frame is `[opcode: u32 LE][length: u32 LE][payload: UTF-8 JSON]`. Every command carries a
//! `nonce` that Discord echoes back in its response, which is what lets a single reader
//! distinguish a reply from an unsolicited event — see [`ResponseKind`].

use rand::Rng;
use rand::distr::Alphanumeric;
use serde::Serialize;
use serde_json::{Value, json};

use crate::error::DiscordError;

/// Length of the frame header: opcode plus payload length.
pub const HEADER_LEN: usize = 8;

/// Refuses absurd frame lengths rather than trying to allocate them. Discord's largest realistic
/// payload is `GET_VOICE_SETTINGS`, a few KB at most.
const MAX_PAYLOAD_LEN: u32 = 8 * 1024 * 1024;

#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeMessage {
    pub opcode: Opcode,
    pub payload: String,
}

impl PipeMessage {
    pub fn new(opcode: Opcode, payload: impl Into<String>) -> Self {
        Self {
            opcode,
            payload: payload.into(),
        }
    }

    /// Serialises the frame. Built from a `serde_json::Value` rather than string formatting, so a
    /// quote inside a token or client id cannot corrupt the payload.
    fn from_json(opcode: Opcode, payload: &impl Serialize) -> Result<Self, DiscordError> {
        Ok(Self {
            opcode,
            payload: serde_json::to_string(payload)?,
        })
    }

    pub fn handshake(client_id: &str) -> Result<Self, DiscordError> {
        Self::from_json(
            Opcode::Handshake,
            &json!({ "v": 1, "client_id": client_id }),
        )
    }

    pub fn pong() -> Result<Self, DiscordError> {
        Self::from_json(Opcode::Pong, &json!({}))
    }

    /// Builds a command frame. The caller owns the nonce so it can register a waiter for the
    /// reply before the frame goes out.
    pub fn command(cmd: &str, nonce: &str, args: Option<Value>) -> Result<Self, DiscordError> {
        let mut payload = json!({ "cmd": cmd, "nonce": nonce });
        if let Some(args) = args {
            payload["args"] = args;
        }
        Self::from_json(Opcode::Frame, &payload)
    }

    /// Builds a `SUBSCRIBE` / `UNSUBSCRIBE` frame, where the event name travels in `evt`.
    pub fn subscription(
        cmd: &str,
        event: &str,
        nonce: &str,
        args: Option<Value>,
    ) -> Result<Self, DiscordError> {
        let mut payload = json!({ "cmd": cmd, "evt": event, "nonce": nonce });
        if let Some(args) = args {
            payload["args"] = args;
        }
        Self::from_json(Opcode::Frame, &payload)
    }

    pub fn to_buff(&self) -> Vec<u8> {
        let bytes = self.payload.as_bytes();
        let mut message = Vec::with_capacity(HEADER_LEN + bytes.len());
        message.extend(&(self.opcode as u32).to_le_bytes());
        message.extend(&(bytes.len() as u32).to_le_bytes());
        message.extend(bytes);
        message
    }

    /// Reads the header, returning the opcode and how many payload bytes follow.
    pub fn parse_header(header: [u8; HEADER_LEN]) -> Result<(Opcode, u32), DiscordError> {
        // Split into two fixed arrays so the conversion cannot fail, rather than slicing and
        // asserting the length back.
        let (opcode_bytes, length_bytes) = header.split_at(4);
        let opcode = Opcode::new(u32::from_le_bytes([
            opcode_bytes[0],
            opcode_bytes[1],
            opcode_bytes[2],
            opcode_bytes[3],
        ]));
        let length = u32::from_le_bytes([
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
        ]);

        if length > MAX_PAYLOAD_LEN {
            return Err(DiscordError::FrameTooLarge(length));
        }
        Ok((opcode, length))
    }

    /// Classifies a decoded frame so the reader can route it.
    pub fn classify(&self) -> Result<ResponseKind, DiscordError> {
        let payload: Value = serde_json::from_str(&self.payload)?;

        let nonce = payload
            .get("nonce")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let event = payload
            .get("evt")
            .and_then(Value::as_str)
            .map(str::to_owned);

        Ok(match (nonce, event) {
            // Errors carry both a nonce and `evt: "ERROR"`; they still answer a command.
            (Some(nonce), _) => ResponseKind::Response { nonce, payload },
            (None, Some(event)) => ResponseKind::Event { event, payload },
            (None, None) => ResponseKind::Unsolicited(payload),
        })
    }
}

/// What a frame from Discord actually is.
///
/// Discord replies and pushes events over the same opcode, so the distinction is made by fields:
/// a `nonce` means it answers a command we sent, `evt` without a nonce means a subscription
/// event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseKind {
    Response {
        nonce: String,
        payload: Value,
    },
    Event {
        event: String,
        payload: Value,
    },
    /// Neither a reply nor a subscription event. `READY` arrives this way after the handshake.
    Unsolicited(Value),
}

/// Extracts Discord's error description from a payload, if it is one.
///
/// Errors arrive as `{"evt": "ERROR", "data": {"code": 4006, "message": "..."}}`. The previous
/// implementation only checked whether `evt` was null and discarded the reason, which turned
/// every failure into the same opaque message.
pub fn error_message(payload: &Value) -> Option<String> {
    if payload.get("evt").and_then(Value::as_str) != Some("ERROR") {
        return None;
    }
    let data = payload.get("data");
    let message = data
        .and_then(|d| d.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("unknown error");
    match data.and_then(|d| d.get("code")).and_then(Value::as_i64) {
        Some(code) => Some(format!("{message} (code {code})")),
        None => Some(message.to_owned()),
    }
}

pub fn generate_nonce() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_through_the_header() {
        let message = PipeMessage::handshake("123456789").expect("builds");
        let buffer = message.to_buff();

        let header: [u8; HEADER_LEN] = buffer[..HEADER_LEN].try_into().expect("header");
        let (opcode, length) = PipeMessage::parse_header(header).expect("parses");

        assert_eq!(opcode, Opcode::Handshake);
        assert_eq!(length as usize, buffer.len() - HEADER_LEN);
        assert_eq!(
            String::from_utf8(buffer[HEADER_LEN..].to_vec()).expect("utf8"),
            message.payload
        );
    }

    #[test]
    fn a_quote_in_a_credential_cannot_corrupt_the_payload() {
        // The previous implementation built JSON with format!, so this input produced a broken
        // frame instead of an escaped string.
        let message = PipeMessage::handshake(r#"evil","v":9,"x":"#).expect("builds");
        let parsed: Value = serde_json::from_str(&message.payload).expect("still valid JSON");

        assert_eq!(parsed["v"], 1);
        assert_eq!(parsed["client_id"], r#"evil","v":9,"x":"#);
    }

    #[test]
    fn oversized_frames_are_rejected_instead_of_allocated() {
        let mut header = [0u8; HEADER_LEN];
        header[0..4].copy_from_slice(&1u32.to_le_bytes());
        header[4..8].copy_from_slice(&u32::MAX.to_le_bytes());

        assert!(matches!(
            PipeMessage::parse_header(header),
            Err(DiscordError::FrameTooLarge(_))
        ));
    }

    #[test]
    fn a_reply_is_told_apart_from_an_event() {
        let response = PipeMessage::new(
            Opcode::Frame,
            r#"{"cmd":"GET_VOICE_SETTINGS","nonce":"abc","data":{"mute":true}}"#,
        );
        assert!(matches!(
            response.classify().expect("classifies"),
            ResponseKind::Response { ref nonce, .. } if nonce == "abc"
        ));

        let event = PipeMessage::new(
            Opcode::Frame,
            r#"{"cmd":"DISPATCH","evt":"VOICE_SETTINGS_UPDATE","data":{"mute":true}}"#,
        );
        assert!(matches!(
            event.classify().expect("classifies"),
            ResponseKind::Event { ref event, .. } if event == "VOICE_SETTINGS_UPDATE"
        ));
    }

    #[test]
    fn an_error_reply_is_routed_to_its_command_not_treated_as_an_event() {
        // Errors carry `evt: "ERROR"` *and* a nonce. Routing them as events would leave the
        // command that failed waiting forever.
        let frame = PipeMessage::new(
            Opcode::Frame,
            r#"{"cmd":"AUTHENTICATE","evt":"ERROR","nonce":"xyz","data":{"code":4009,"message":"Invalid token"}}"#,
        );

        let ResponseKind::Response { nonce, payload } = frame.classify().expect("classifies")
        else {
            panic!("an error carrying a nonce must be routed as a response");
        };

        assert_eq!(nonce, "xyz");
        assert_eq!(
            error_message(&payload).as_deref(),
            Some("Invalid token (code 4009)")
        );
    }

    #[test]
    fn successful_payloads_carry_no_error_message() {
        let payload: Value = serde_json::from_str(r#"{"cmd":"AUTHENTICATE","data":{}}"#).unwrap();
        assert!(error_message(&payload).is_none());
    }

    #[test]
    fn nonces_are_unique_per_command() {
        let first = generate_nonce();
        let second = generate_nonce();
        assert_ne!(first, second);
        assert_eq!(first.len(), 32);
    }
}
