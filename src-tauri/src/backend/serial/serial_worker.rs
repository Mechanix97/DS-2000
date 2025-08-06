use crate::error::SerialPortError;
use crate::serial_state::InMessage;
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

    pub async fn start(&mut self) -> Result<(), SerialPortError> {
        debug!("Serial worker started");
        self.serial_port_handler
            .cast(InMessage::Fetch)
            .await
            .map_err(|e| SerialPortError::GenServerError(e))
    }
}
