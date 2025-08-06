use crate::error::SerialPortError;
use crate::serial_state::InCallMessage;
use crate::serial_state::InMessage;
use crate::serial_state::OutMessage;
use crate::serial_state::SerialPortHandler;
use crate::serial_state::SerialPortState;

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
            .map_err(|e| SerialPortError::GenServerError(e))
    }

    pub async fn get_port_name(&mut self) -> Result<Option<String>, SerialPortError> {
        let om: OutMessage = self
            .serial_port_handler
            .call(InCallMessage::PortName)
            .await
            .map_err(|e| SerialPortError::GenServerError(e))?;
        let OutMessage::PortName(rt) = om else {
            return Ok(None);
        };
        Ok(rt)
    }
}
