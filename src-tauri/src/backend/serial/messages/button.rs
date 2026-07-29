use bytes::BufMut;

use crate::error::SerialMessageError;
use crate::serial_message::SerialFrame;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Button {
    MuteButton,
    DeafenButton,
    DisconnectButton,
}

impl Button {
    const fn as_id(self) -> u8 {
        match self {
            Button::MuteButton => 0,
            Button::DeafenButton => 1,
            Button::DisconnectButton => 2,
        }
    }

    /// Decodes a button id off the wire.
    ///
    /// Returns an error rather than panicking: this byte comes from a serial cable, so a single
    /// glitch used to take down the whole application.
    const fn from_u8(id: u8) -> Result<Self, SerialMessageError> {
        match id {
            0 => Ok(Button::MuteButton),
            1 => Ok(Button::DeafenButton),
            2 => Ok(Button::DisconnectButton),
            _ => Err(SerialMessageError::MalformedData),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ButtonMessage {
    pub button: Button,
}

impl SerialFrame for ButtonMessage {
    const CODE: u8 = 0x02;

    fn encode(&self, buf: &mut dyn BufMut) -> Result<(), SerialMessageError> {
        buf.put_u8(self.button.as_id());
        Ok(())
    }

    fn decode(msg_data: &[u8]) -> Result<Self, SerialMessageError> {
        let id = msg_data
            .first()
            .ok_or(SerialMessageError::InvalidMessageLength)?;
        Ok(Self {
            button: Button::from_u8(*id)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_button_survives_a_round_trip() {
        for button in [
            Button::MuteButton,
            Button::DeafenButton,
            Button::DisconnectButton,
        ] {
            let mut buffer = Vec::new();
            ButtonMessage { button }
                .encode(&mut buffer)
                .expect("encodes");

            assert_eq!(
                ButtonMessage::decode(&buffer).expect("decodes").button,
                button
            );
        }
    }

    #[test]
    fn an_unknown_button_id_is_an_error_not_a_panic() {
        // One flipped bit on the cable used to abort the process here.
        assert_eq!(
            ButtonMessage::decode(&[0x07]),
            Err(SerialMessageError::MalformedData)
        );
    }

    #[test]
    fn an_empty_payload_is_an_error_not_an_index_out_of_bounds() {
        assert_eq!(
            ButtonMessage::decode(&[]),
            Err(SerialMessageError::InvalidMessageLength)
        );
    }
}
