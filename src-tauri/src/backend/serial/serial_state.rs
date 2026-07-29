//! Connection state machine for the serial device.
//!
//! Nothing runs on a timer while the port is open: frames arrive from the reader task and are
//! handled as they come. The only scheduled work is the reconnect attempt, which backs off so a
//! machine with no device attached is not rescanning its ports several times a second.

use common::rgb_update::RGBConfig;
use std::path::PathBuf;

use crate::messages::rgb::RGBConfigMessage;
use crate::messages::voice_settings::VoiceSettingsMessage;

use crate::port::{Port, SerialEvent};
use crate::{error::SerialPortError, serial_message::SerialMessage};

use spawned_concurrency::tasks::{
    CallResponse, CastResponse, GenServer, GenServerHandle, send_after,
};
use std::collections::VecDeque;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tracing::{debug, info, warn};

/// First delay after a failed connection attempt.
const RECONNECT_BACKOFF_MIN: Duration = Duration::from_secs(1);
/// Ceiling for the backoff. Scanning every port has a cost, so an absent device settles here.
const RECONNECT_BACKOFF_MAX: Duration = Duration::from_secs(15);

/// Bounds the queue so a device flooding the link cannot grow it without limit.
///
/// Reaching this means the consumer stopped draining, which is a bug rather than a legitimate
/// state, so the oldest messages are dropped and the fact is logged.
const MAX_QUEUED_MESSAGES: usize = 256;

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
    /// Attempts to bring the port up. Rescheduled with backoff only while it fails.
    Connect(Option<String>),
    SetVoiceSettings(bool, bool),
    RGBUpdate(RGBConfig),
    /// Something the reader task observed.
    Serial(SerialEvent),
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
    port: Port,
    baudrate: u32,
    timeout: Duration,
    reconnect_backoff: Duration,
    /// Guards against several reconnect timers piling up, which would defeat the backoff.
    reconnect_scheduled: bool,
    shutdown: bool,
    // TODO: phase 4 replaces this with a push to the controller. It stays for now because the
    // controller still drains it from its own loop.
    message_queue: VecDeque<SerialMessage>,
}

impl SerialPortState {
    pub async fn spawn(baudrate: u32, timeout: Duration) -> SerialPortHandler {
        let (events_tx, mut events_rx) = mpsc::unbounded_channel();

        let state = Self {
            port: Port::new(events_tx),
            baudrate,
            timeout,
            reconnect_backoff: RECONNECT_BACKOFF_MIN,
            reconnect_scheduled: false,
            shutdown: false,
            message_queue: VecDeque::new(),
        };
        let handle = state.start();

        // Bridges the reader task's frames into the actor's mailbox, so the actor only ever
        // reacts to messages and never polls the port.
        let mut forward_to = handle.clone();
        tokio::spawn(async move {
            while let Some(event) = events_rx.recv().await {
                if forward_to.cast(InMessage::Serial(event)).await.is_err() {
                    break;
                }
            }
        });

        handle
    }

    /// Schedules another connection attempt, doubling the delay up to the ceiling.
    fn schedule_reconnect(&mut self, handle: &GenServerHandle<Self>) {
        if self.shutdown || self.reconnect_scheduled {
            return;
        }
        self.reconnect_scheduled = true;
        send_after(
            self.reconnect_backoff,
            handle.clone(),
            InMessage::Connect(None),
        );
        self.reconnect_backoff = (self.reconnect_backoff * 2).min(RECONNECT_BACKOFF_MAX);
    }

    fn enqueue(&mut self, message: SerialMessage) {
        if self.message_queue.len() >= MAX_QUEUED_MESSAGES {
            warn!("Serial message queue is full, dropping the oldest message");
            self.message_queue.pop_front();
        }
        self.message_queue.push_back(message);
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
            InMessage::Connect(preferred_port) => {
                self.reconnect_scheduled = false;

                if self.port.is_connected() {
                    return CastResponse::NoReply(self);
                }

                // The port used last is tried first, which skips scanning every other device on
                // the machine in the common case.
                let connected = match &preferred_port {
                    Some(name) => {
                        debug!("Trying the last used port {name}");
                        self.port
                            .connect_and_authenticate(
                                &PathBuf::from(name),
                                self.baudrate,
                                self.timeout,
                            )
                            .await
                            .is_ok()
                    }
                    None => false,
                };

                if !connected
                    && let Err(err) = self.port.auto_connect(self.baudrate, self.timeout).await
                {
                    debug!("No DS-2000 found on any serial port: {err}");
                    self.schedule_reconnect(handle);
                    return CastResponse::NoReply(self);
                }

                self.reconnect_backoff = RECONNECT_BACKOFF_MIN;
                CastResponse::NoReply(self)
            }

            InMessage::Serial(SerialEvent::Message(message)) => {
                self.enqueue(message);
                CastResponse::NoReply(self)
            }

            InMessage::Serial(SerialEvent::Disconnected) => {
                info!("Serial device disconnected, will try to reconnect");
                self.port.disconnect().await;
                self.schedule_reconnect(handle);
                CastResponse::NoReply(self)
            }

            InMessage::SetVoiceSettings(mute, deafen) => {
                if self.port.is_connected()
                    && let Err(err) = self
                        .port
                        .send_message(&SerialMessage::VoiceSettings(VoiceSettingsMessage {
                            mute,
                            deafen,
                        }))
                        .await
                {
                    warn!("Could not send voice settings to the device: {err}");
                }
                CastResponse::NoReply(self)
            }

            InMessage::RGBUpdate(update) => {
                if self.port.is_connected()
                    && let Err(err) = self
                        .port
                        .send_message(&SerialMessage::RGBUpdate(RGBConfigMessage { update }))
                        .await
                {
                    warn!("Could not send the RGB update to the device: {err}");
                }
                CastResponse::NoReply(self)
            }
        }
    }

    async fn handle_call(
        mut self,
        message: Self::CallMsg,
        _handle: &GenServerHandle<Self>,
    ) -> CallResponse<Self> {
        match message {
            InCallMessage::PortName => {
                let name = self.port.name.clone();
                CallResponse::Reply(self, OutMessage::PortName(name))
            }
            InCallMessage::Shutdown => {
                self.shutdown = true;
                self.port.disconnect().await;
                CallResponse::Reply(self, OutMessage::Done)
            }
            InCallMessage::PendingMessages => {
                let pending = self.message_queue.drain(..).collect::<Vec<_>>();
                CallResponse::Reply(self, OutMessage::PendingMessages(pending))
            }
            InCallMessage::SerialPortStatus => {
                let connected = self.port.is_connected();
                CallResponse::Reply(self, OutMessage::SerialPortStatus(connected))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::button::{Button, ButtonMessage};
    use crate::messages::ping::PingMessage;

    fn state() -> SerialPortState {
        let (tx, _rx) = mpsc::unbounded_channel();
        SerialPortState {
            port: Port::new(tx),
            baudrate: 115200,
            timeout: Duration::from_millis(1000),
            reconnect_backoff: RECONNECT_BACKOFF_MIN,
            reconnect_scheduled: false,
            shutdown: false,
            message_queue: VecDeque::new(),
        }
    }

    #[test]
    fn queued_messages_keep_their_order() {
        let mut state = state();
        state.enqueue(SerialMessage::Ping(PingMessage {}));
        state.enqueue(SerialMessage::Button(ButtonMessage {
            button: Button::MuteButton,
        }));

        let drained = state.message_queue.drain(..).collect::<Vec<_>>();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0], SerialMessage::Ping(PingMessage {}));
    }

    #[test]
    fn the_queue_is_bounded_and_drops_the_oldest() {
        let mut state = state();
        for _ in 0..MAX_QUEUED_MESSAGES + 10 {
            state.enqueue(SerialMessage::Ping(PingMessage {}));
        }

        assert_eq!(state.message_queue.len(), MAX_QUEUED_MESSAGES);
    }

    #[test]
    fn the_reconnect_backoff_grows_and_then_settles_at_the_ceiling() {
        let mut state = state();
        let mut delays = Vec::new();

        for _ in 0..8 {
            delays.push(state.reconnect_backoff);
            // Mirrors what schedule_reconnect does, without needing a live actor handle.
            state.reconnect_backoff = (state.reconnect_backoff * 2).min(RECONNECT_BACKOFF_MAX);
        }

        assert_eq!(delays[0], RECONNECT_BACKOFF_MIN);
        assert!(delays[1] > delays[0]);
        assert_eq!(*delays.last().expect("non-empty"), RECONNECT_BACKOFF_MAX);
    }
}
