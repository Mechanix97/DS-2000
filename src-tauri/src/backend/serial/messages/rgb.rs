use bytes::BufMut;
use common::rgb_update::RGBConfig;
use common::rgb_update::RGBMode;

use crate::error::SerialMessageError;
use crate::serial_message::RLPxMessage;

#[derive(Debug, PartialEq, Eq, Clone)]

pub struct RGBConfigMessage {
    pub update: RGBConfig,
}

impl RLPxMessage for RGBConfigMessage {
    const CODE: u8 = 0x04;
    fn encode(&self, buf: &mut dyn BufMut) -> Result<(), SerialMessageError> {
        buf.put_u8(self.update.brightness);
        match &self.update.rgb_mode {
            RGBMode::Cycle => {
                buf.put_u8(0x00);
            }
            RGBMode::Fixed { led1, led2 } => {
                buf.put_u8(0x01);
                buf.put_u8(led1.red);
                buf.put_u8(led1.green);
                buf.put_u8(led1.blue);
                buf.put_u8(led2.red);
                buf.put_u8(led2.green);
                buf.put_u8(led2.blue);
            }
            RGBMode::Wave { led1, led2 } => {
                buf.put_u8(0x02);
                buf.put_u8(led1.red);
                buf.put_u8(led1.green);
                buf.put_u8(led1.blue);
                buf.put_u8(led2.red);
                buf.put_u8(led2.green);
                buf.put_u8(led2.blue);
            }
        }
        Ok(())
    }

    fn decode(_msg_data: &[u8]) -> Result<Self, SerialMessageError> {
        Ok(Self {
            update: RGBConfig::default(),
        })
    }
}
