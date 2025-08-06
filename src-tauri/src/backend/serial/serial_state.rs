use std::path::PathBuf;

use crate::error::SerialPortError;
use crate::port::Port;

use spawned_concurrency::tasks::{
    CallResponse, CastResponse, GenServer, GenServerHandle, send_after,
};
use tokio::time::Duration;
use tracing::debug;

const SERIAL_FETCH_INTERVAL: u64 = 50; //millis
const SERIAL_AUTOCONNECT_INTERVAL: u64 = 30; //secs

pub type SerialPortHandler = GenServerHandle<SerialPortState>;

#[derive(Clone)]
pub enum InCallMessage {
    PortName,
    Shutdown,
}

#[derive(Clone)]
pub enum InMessage {
    Fetch,
    Start(Option<String>),
}

#[derive(Clone, PartialEq)]
pub enum OutMessage {
    Done,
    PortName(Option<String>),
}

#[derive(Clone)]
pub struct SerialPortState {
    fetch_interval_ms: u64,
    autoconnect_interval_ms: u64,
    port: Port,
    baudrate: u32,
    timeout: Duration,
    shutdown: bool,
}

impl SerialPortState {
    pub fn new(baudrate: u32, timeout: Duration) -> Self {
        Self {
            fetch_interval_ms: SERIAL_FETCH_INTERVAL,
            autoconnect_interval_ms: SERIAL_AUTOCONNECT_INTERVAL,
            port: Port::new(),
            baudrate,
            timeout,
            shutdown: false,
        }
    }

    pub async fn spawn(baudrate: u32, timeout: Duration) -> SerialPortHandler {
        let state = Self::new(baudrate, timeout);
        state.start()
    }
}

impl GenServer for SerialPortState {
    type CallMsg = InCallMessage;
    type CastMsg = InMessage;
    type OutMsg = OutMessage;
    type Error = SerialPortError;

    async fn handle_cast(
        mut self,
        message: Self::CastMsg,
        handle: &GenServerHandle<Self>,
    ) -> CastResponse<Self> {
        if self.shutdown {
            return CastResponse::NoReply(self);
        }
        match message {
            InMessage::Start(port) => {
                if let Some(port_name) = port {
                    debug!("Connecting to last used port {port_name}");
                    if let Err(err) =
                        self.port
                            .connect(&PathBuf::from(&port_name), self.baudrate, self.timeout)
                    {
                        debug!("Error connecting to given port {port_name}: {err}");
                    }
                }

                send_after(
                    Duration::from_millis(1),
                    handle.clone(),
                    Self::CastMsg::Fetch,
                );
            }
            InMessage::Fetch => {
                if !self.port.is_connected() {
                    if let Err(err) = self.port.auto_connect(self.baudrate, self.timeout).await {
                        debug!("Error serial auto connecting: {err}");
                        send_after(
                            Duration::from_secs(self.autoconnect_interval_ms),
                            handle.clone(),
                            Self::CastMsg::Fetch,
                        );
                        return CastResponse::NoReply(self);
                    }
                }

                send_after(
                    Duration::from_millis(self.fetch_interval_ms),
                    handle.clone(),
                    Self::CastMsg::Fetch,
                );
            }
        }
        CastResponse::NoReply(self)
    }

    async fn handle_call(
        mut self,
        message: Self::CallMsg,
        _handle: &GenServerHandle<Self>,
    ) -> CallResponse<Self> {
        match message {
            Self::CallMsg::PortName => {
                let pn = self.port.name.clone();
                CallResponse::Reply(self, OutMessage::PortName(pn))
            }
            Self::CallMsg::Shutdown => {
                self.shutdown = true;
                if let Err(err) = self.port.disconnect().await {
                    debug!("Error disconnecting serial port: {err}");
                }
                CallResponse::Reply(self, OutMessage::Done)
            }
        }
    }
}

// for running these tests, a device should be connected
#[cfg(test)]
mod tests {}
