use bytes::BufMut;

use crate::error::SerialMessageError;
use crate::serial_message::SerialFrame;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PingMessage {}

impl SerialFrame for PingMessage {
    const CODE: u8 = 0x00;

    /// A ping has no payload.
    ///
    /// This used to write `0xFF` here, which is the frame delimiter the codec appends itself. The
    /// result was every ping going out as `[CODE, 0xFF, 0xFF]` — a ping followed by an empty
    /// frame — and a stray byte left in the read buffer that desynchronised the next frame.
    /// Framing belongs to the codec, not to a message body.
    fn encode(&self, _buf: &mut dyn BufMut) -> Result<(), SerialMessageError> {
        Ok(())
    }

    fn decode(_msg_data: &[u8]) -> Result<Self, SerialMessageError> {
        Ok(Self {})
    }
}
