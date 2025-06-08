use bytes::BufMut;

use super::error::SerialMessageError;
use super::messages::ping::PingMessage;
use super::messages::pong::PongMessage;

#[derive(Debug)]
pub enum SerialMessage {
    Ping(PingMessage),
    Pong(PongMessage),
}

impl SerialMessage {
    fn code(&self) -> u8 {
        match self {
            SerialMessage::Ping(_) => PingMessage::CODE,
            SerialMessage::Pong(_) => PongMessage::CODE,
        }
    }

    pub fn decode(data: &[u8]) -> Result<SerialMessage, SerialMessageError> {
        let msg_id = data[0];
        match msg_id {
            PingMessage::CODE => Ok(SerialMessage::Ping(PingMessage::decode(data)?)),
            PongMessage::CODE => Ok(SerialMessage::Pong(PongMessage::decode(data)?)),
            _ => Err(SerialMessageError::MalformedData),
        }
    }

    pub fn encode(&self, buf: &mut dyn BufMut) -> Result<(), SerialMessageError> {
        match self {
            SerialMessage::Ping(msg) => msg.encode(buf),
            SerialMessage::Pong(msg) => msg.encode(buf),
        }
    }
}

pub trait RLPxMessage: Sized {
    const CODE: u8;

    fn encode(&self, buf: &mut dyn BufMut) -> Result<(), SerialMessageError>;

    fn decode(msg_data: &[u8]) -> Result<Self, SerialMessageError>;
}
