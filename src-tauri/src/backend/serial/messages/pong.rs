use bytes::BufMut;

use crate::error::SerialMessageError;
use crate::serial_message::RLPxMessage;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PongMessage {}

impl RLPxMessage for PongMessage {
    const CODE: u8 = 0x01;
    fn encode(&self, buf: &mut dyn BufMut) -> Result<(), SerialMessageError> {
        buf.put_u8(0xFF);
        Ok(())
    }

    fn decode(_msg_data: &[u8]) -> Result<Self, SerialMessageError> {
        Ok(Self {})
    }
}
