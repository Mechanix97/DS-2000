use bytes::BufMut;

use crate::error::SerialMessageError;
use crate::serial_message::SerialFrame;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PongMessage {}

impl SerialFrame for PongMessage {
    const CODE: u8 = 0x01;

    /// A pong has no payload. See [`crate::messages::ping::PingMessage::encode`] for why the
    /// delimiter byte that used to be written here was wrong.
    fn encode(&self, _buf: &mut dyn BufMut) -> Result<(), SerialMessageError> {
        Ok(())
    }

    fn decode(_msg_data: &[u8]) -> Result<Self, SerialMessageError> {
        Ok(Self {})
    }
}
