use std::path::PathBuf;

use crate::messages::voice_settings::VoiceSettingsMessage;
use crate::port::Port;
use crate::{error::SerialPortError, serial_message::SerialMessage};

use spawned_concurrency::tasks::{
    CallResponse, CastResponse, GenServer, GenServerHandle, send_after,
};
use std::collections::VecDeque;
use tokio::time::Duration;
use tracing::debug;
use tracing::info;

const SERIAL_FETCH_INTERVAL: u64 = 50; //millis
const SERIAL_AUTOCONNECT_INTERVAL: u64 = 15; //secs

pub type SerialPortHandler = GenServerHandle<SerialPortState>;

#[derive(Clone)]
pub enum InCallMessage {
    PortName,
    Shutdown,
    PendingMessages,
    SerialPortStatus,
}

#[derive(Clone)]
pub enum InMessage {
    Fetch,
    Start(Option<String>),
    SetVoiceSettings(bool, bool),
}

#[derive(Clone, PartialEq)]
pub enum OutMessage {
    Done,
    PortName(Option<String>),
    PendingMessages(Vec<SerialMessage>),
    SerialPortStatus(bool),
}

#[derive(Clone)]
pub struct SerialPortState {
    fetch_interval_ms: u64,
    autoconnect_interval_ms: u64,
    port: Port,
    baudrate: u32,
    timeout: Duration,
    shutdown: bool,
    message_queue: VecDeque<SerialMessage>,
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
            message_queue: VecDeque::new(),
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
                    if let Err(err) = self
                        .port
                        .connect_and_authenticate(
                            &PathBuf::from(&port_name),
                            self.baudrate,
                            self.timeout,
                        )
                        .await
                    {
                        debug!("Error connecting to given port {port_name}: {err}");
                    }
                }
                send_after(Duration::from_secs(1), handle.clone(), Self::CastMsg::Fetch);
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

                loop {
                    match self.port.read_message(Duration::from_millis(100)).await {
                        Ok(msg) => {
                            info!("message received: {msg:?}");
                            self.message_queue.push_back(msg);
                        }
                        Err(err) => {
                            if err == SerialPortError::TimedOut {
                                break;
                            }
                            if let Err(err) = self.port.disconnect().await {
                                debug!("Error disconnecting port {err}");
                            }
                            break;
                        }
                    }
                }

                send_after(
                    Duration::from_millis(self.fetch_interval_ms),
                    handle.clone(),
                    Self::CastMsg::Fetch,
                );
            }
            InMessage::SetVoiceSettings(mute, deafen) => {
                if let Err(err) = self
                    .port
                    .send_message(&SerialMessage::VoiceSettings(VoiceSettingsMessage {
                        mute,
                        deafen,
                    }))
                    .await
                {
                    debug!("Error sending voice settings msg: {err}");
                }
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
            Self::CallMsg::PendingMessages => {
                let pending = self.message_queue.drain(0..).collect::<Vec<_>>();

                CallResponse::Reply(self, OutMessage::PendingMessages(pending))
            }
            Self::CallMsg::SerialPortStatus => {
                let st = self.port.is_connected();
                CallResponse::Reply(self, OutMessage::SerialPortStatus(st))
            }
        }
    }
}

// for running these tests, a device should be connected
#[cfg(test)]
mod tests {}
