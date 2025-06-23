use bytes::{BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

use super::error::{SerialMessageError, SerialPortError};
use super::messages::ping::PingMessage;
use super::messages::pong::PongMessage;

pub struct SerialMessageCodec;

impl Decoder for SerialMessageCodec {
    type Item = SerialMessage;
    type Error = SerialPortError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, SerialPortError> {
        // Buscamos el delimitador 0xFF
        if let Some(pos) = src.iter().position(|&b| b == 0xFF) {
            // Extraemos hasta el delimitador incluido
            let frame = src.split_to(pos + 1);

            // Removemos el delimitador para decodificar solo el contenido
            let data = &frame[..frame.len() - 1];

            // Decodificamos el mensaje (propagando el error si falla)
            Ok(Some(SerialMessage::decode(data)?))
        } else {
            // Si no encontramos delimitador, seguimos leyendo
            Ok(None)
        }
    }
}

impl Encoder<SerialMessage> for SerialMessageCodec {
    type Error = SerialPortError;

    fn encode(&mut self, item: SerialMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        // Codificamos el mensaje en el buffer
        item.encode(dst)?;

        // Agregamos el delimitador 0xFF al final
        dst.put_u8(0xFF);

        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
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
        if data.is_empty() {
            return Err(SerialMessageError::MalformedData);
        }

        let msg_id = data[0];
        match msg_id {
            PingMessage::CODE => Ok(SerialMessage::Ping(PingMessage::decode(data)?)),
            PongMessage::CODE => Ok(SerialMessage::Pong(PongMessage::decode(data)?)),
            _ => Err(SerialMessageError::MalformedData),
        }
    }

    pub fn encode(&self, buf: &mut dyn BufMut) -> Result<(), SerialMessageError> {
        buf.put_u8(self.code());
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
