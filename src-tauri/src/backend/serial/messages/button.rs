use bytes::BufMut;

use crate::error::SerialMessageError;
use crate::serial_message::RLPxMessage;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Button {
    MuteButton,
    DeafenButton,
    DisconnectButton,
}

impl Button {
    fn as_id(&self) -> u8 {
        match self {
            Button::MuteButton => 0,
            Button::DeafenButton => 1,
            Button::DisconnectButton => 2,
        }
    }

    fn from_u8(id: u8) -> Self {
        match id {
            0 => Button::MuteButton,
            1 => Button::DeafenButton,
            2 => Button::DisconnectButton,
            _ => panic!("Invalid button"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ButtonMessage {
    pub button: Button,
}

impl RLPxMessage for ButtonMessage {
    const CODE: u8 = 0x02;
    fn encode(&self, buf: &mut dyn BufMut) -> Result<(), SerialMessageError> {
        buf.put_u8(self.button.as_id());
        Ok(())
    }

    fn decode(msg_data: &[u8]) -> Result<Self, SerialMessageError> {
        Ok(Self {
            button: Button::from_u8(msg_data[0]),
        })
    }
}
