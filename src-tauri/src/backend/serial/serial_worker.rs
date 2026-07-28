use crate::error::SerialPortError;
use crate::serial_message::SerialMessage;
use crate::serial_state::InCallMessage;
use crate::serial_state::InMessage;
use crate::serial_state::OutMessage;
use crate::serial_state::SerialPortHandler;
use crate::serial_state::SerialPortState;

use common::rgb_update::RGBConfig;

use tokio::time::Duration;
use tracing::debug;

pub struct SerialWorker {
    serial_port_handler: SerialPortHandler,
}

impl SerialWorker {
    pub async fn new(baudrate: u32, timeout: Duration) -> Self {
        let serial_port_handler = SerialPortState::spawn(baudrate, timeout).await;

        Self {
            serial_port_handler,
        }
    }

    pub async fn start(&mut self, last_used_port: Option<String>) -> Result<(), SerialPortError> {
        debug!("Serial worker started");
        self.serial_port_handler
            .cast(InMessage::Start(last_used_port))
            .await
            .map_err(SerialPortError::GenServerError)
    }

    pub async fn get_port_name(&mut self) -> Result<Option<String>, SerialPortError> {
        let om: OutMessage = self
            .serial_port_handler
            .call(InCallMessage::PortName)
            .await
            .map_err(SerialPortError::GenServerError)?;
        let OutMessage::PortName(rt) = om else {
            return Ok(None);
        };
        Ok(rt)
    }

    pub async fn get_pending_messages(&mut self) -> Result<Vec<SerialMessage>, SerialPortError> {
        let om: OutMessage = self
            .serial_port_handler
            .call(InCallMessage::PendingMessages)
            .await
            .map_err(SerialPortError::GenServerError)?;
        let OutMessage::PendingMessages(pm) = om else {
            return Ok(vec![]);
        };
        Ok(pm)
    }

    pub async fn set_voice_settings(
        &mut self,
        mute: bool,
        deafen: bool,
    ) -> Result<(), SerialPortError> {
        self.serial_port_handler
            .cast(InMessage::SetVoiceSettings(mute, deafen))
            .await
            .map_err(SerialPortError::GenServerError)?;
        Ok(())
    }

    pub async fn is_connected(&mut self) -> Result<bool, SerialPortError> {
        let om: OutMessage = self
            .serial_port_handler
            .call(InCallMessage::SerialPortStatus)
            .await
            .map_err(SerialPortError::GenServerError)?;
        if let OutMessage::SerialPortStatus(st) = om {
            return Ok(st);
        }
        Ok(false)
    }

    pub async fn shutdown(&mut self) -> Result<(), SerialPortError> {
        debug!("Shutting down serial worker");
        self.serial_port_handler
            .call(InCallMessage::Shutdown)
            .await
            .map_err(|e: spawned_concurrency::error::GenServerError| {
                SerialPortError::GenServerError(e)
            })?;
        Ok(())
    }

    pub async fn set_rgb_config(&mut self, rgb_update: &RGBConfig) -> Result<(), SerialPortError> {
        self.serial_port_handler
            .cast(InMessage::RGBUpdate(rgb_update.clone()))
            .await
            .map_err(SerialPortError::GenServerError)?;
        Ok(())
    }
}
