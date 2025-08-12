use bytes::BufMut;

use crate::error::SerialMessageError;
use crate::serial_message::RLPxMessage;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct VoiceSettingsMessage {
    pub mute: bool,
    pub deafen: bool,
}

impl RLPxMessage for VoiceSettingsMessage {
    const CODE: u8 = 0x03;
    fn encode(&self, buf: &mut dyn BufMut) -> Result<(), SerialMessageError> {
        buf.put_u8(self.mute as u8);
        buf.put_u8(self.deafen as u8);

        Ok(())
    }

    fn decode(msg_data: &[u8]) -> Result<Self, SerialMessageError> {
        if msg_data.len() < 2 {
            return Err(SerialMessageError::InvalidMessageLength);
        }
        Ok(Self {
            mute: msg_data[0] != 0,
            deafen: msg_data[1] != 0,
        })
    }
}
