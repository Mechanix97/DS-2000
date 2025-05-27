use bytes::BufMut;

use super::error::SerialMessageError;
use super::serial_message::RLPxMessage;

#[derive(Debug)]
pub struct PingMessage {}

impl RLPxMessage for PingMessage {
    const CODE: u8 = 0x00;
    fn encode(&self, buf: &mut dyn BufMut) -> Result<(), SerialMessageError> {
        buf.put_u8(PingMessage::CODE);
        buf.put_u8(0xFF);
        Ok(())
    }

    fn decode(_msg_data: &[u8]) -> Result<Self, SerialMessageError> {
        Ok(Self {})
    }
}
