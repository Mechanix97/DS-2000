//! Client for Discord's local RPC pipe.
//!
//! # Why a reader task
//!
//! Discord multiplexes replies and pushed events over the same connection. The previous
//! implementation wrote a command and then read the *next* frame, assuming it was the reply —
//! which holds only while nothing is subscribed. As soon as `SUBSCRIBE` is used, an event can
//! land between a command and its reply and every subsequent read answers the wrong question.
//!
//! So exactly one task reads the pipe. It routes each frame by nonce: replies wake the command
//! waiting on that nonce, events go to the event channel. Nothing else ever reads.

use common::task_guard::AbortOnDrop;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::time::{Duration, timeout};
use tracing::{debug, info, warn};

#[cfg(windows)]
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};

#[cfg(unix)]
use {std::env::var, tokio::net::UnixStream};

use crate::error::DiscordError;
use crate::pipe_message::{HEADER_LEN, Opcode, PipeMessage, ResponseKind, error_message};

/// The pipe's concrete type, so the rest of the module stays platform independent.
#[cfg(windows)]
type Transport = NamedPipeClient;
#[cfg(unix)]
type Transport = UnixStream;

/// How long a command waits for its reply before giving up.
///
/// Bounded so a dropped or malformed reply cannot park the connection forever; the state machine
/// reconnects instead.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

/// Event Discord pushes once subscribed. Carries the same payload as `GET_VOICE_SETTINGS`.
pub const VOICE_SETTINGS_UPDATE: &str = "VOICE_SETTINGS_UPDATE";

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DiscordConnectionState {
    NotConnected,
    Connected,
    HandshakeDone,
    Authorized,
    Authenticated,
}

/// Something the reader task observed, delivered to the state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcEvent {
    VoiceSettings {
        mute: bool,
        deafen: bool,
    },
    /// The pipe closed or failed. The state machine reconnects with backoff.
    Disconnected,
}

type PendingReplies = Arc<Mutex<HashMap<String, oneshot::Sender<Value>>>>;

#[derive(Clone)]
struct Connection {
    writer: Arc<Mutex<WriteHalf<Transport>>>,
    pending: PendingReplies,
    _reader_task: Arc<AbortOnDrop>,
}

#[derive(Clone)]
pub struct IpcClient {
    connection: Option<Connection>,
    client_id: Option<String>,
    events: mpsc::UnboundedSender<IpcEvent>,
    pub state: DiscordConnectionState,
}

impl IpcClient {
    pub fn new(events: mpsc::UnboundedSender<IpcEvent>) -> Self {
        Self {
            connection: None,
            client_id: None,
            events,
            state: DiscordConnectionState::NotConnected,
        }
    }

    /// Opens the pipe and starts the reader task.
    ///
    /// Discord numbers its pipes 0-9 so several clients can coexist; the first that accepts a
    /// connection wins.
    pub async fn connect(&mut self) -> Result<(), DiscordError> {
        if self.connection.is_some() || self.state != DiscordConnectionState::NotConnected {
            return Err(DiscordError::ClientAlreadyConnected);
        }

        let pipe = open_pipe().await?;
        let (reader, writer) = tokio::io::split(pipe);

        let writer = Arc::new(Mutex::new(writer));
        let pending: PendingReplies = Arc::new(Mutex::new(HashMap::new()));

        let reader_task = tokio::spawn(read_loop(
            reader,
            writer.clone(),
            pending.clone(),
            self.events.clone(),
        ));

        self.connection = Some(Connection {
            writer,
            pending,
            _reader_task: AbortOnDrop::new(reader_task),
        });
        self.state = DiscordConnectionState::Connected;
        Ok(())
    }

    pub async fn disconnect(&mut self) {
        if self.state != DiscordConnectionState::NotConnected {
            info!("Discord disconnected");
        }
        // Dropping the connection aborts the reader task and closes the pipe.
        self.connection = None;
        self.client_id = None;
        self.state = DiscordConnectionState::NotConnected;
    }

    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }

    async fn write_frame(&self, message: PipeMessage) -> Result<(), DiscordError> {
        let connection = self
            .connection
            .as_ref()
            .ok_or(DiscordError::PipeNotConnected)?;
        connection
            .writer
            .lock()
            .await
            .write_all(&message.to_buff())
            .await?;
        Ok(())
    }

    /// Sends a frame carrying a nonce and waits for the matching reply.
    ///
    /// The waiter is registered *before* the frame goes out, otherwise a fast reply could arrive
    /// while there is nothing left to receive it.
    async fn request(
        &self,
        build: impl FnOnce(&str) -> Result<PipeMessage, DiscordError>,
    ) -> Result<Value, DiscordError> {
        let connection = self
            .connection
            .as_ref()
            .ok_or(DiscordError::PipeNotConnected)?;

        let nonce = crate::pipe_message::generate_nonce();
        let message = build(&nonce)?;

        let (tx, rx) = oneshot::channel();
        connection.pending.lock().await.insert(nonce.clone(), tx);

        if let Err(err) = self.write_frame(message).await {
            connection.pending.lock().await.remove(&nonce);
            return Err(err);
        }

        let payload = match timeout(COMMAND_TIMEOUT, rx).await {
            Ok(Ok(payload)) => payload,
            // The sender was dropped: the reader task ended, so the connection is gone.
            Ok(Err(_)) => return Err(DiscordError::PipeNotConnected),
            Err(_) => {
                connection.pending.lock().await.remove(&nonce);
                return Err(DiscordError::CommandTimedOut);
            }
        };

        if let Some(message) = error_message(&payload) {
            return Err(DiscordError::Rpc(message));
        }
        Ok(payload)
    }

    pub async fn handshake(&mut self, client_id: &str) -> Result<(), DiscordError> {
        if self.state != DiscordConnectionState::Connected {
            return Err(DiscordError::ClientNotConnected);
        }
        self.client_id = Some(client_id.to_owned());
        self.write_frame(PipeMessage::handshake(client_id)?).await?;
        self.state = DiscordConnectionState::HandshakeDone;
        Ok(())
    }

    /// Prompts the user with Discord's authorisation modal and returns the OAuth code.
    pub async fn authorize(&mut self, scopes: &str) -> Result<String, DiscordError> {
        if self.state != DiscordConnectionState::HandshakeDone {
            return Err(DiscordError::HandshakeNotDone);
        }
        let client_id = self
            .client_id
            .clone()
            .ok_or(DiscordError::ClientIdNotFound)?;

        let payload = self
            .request(|nonce| {
                PipeMessage::command(
                    "AUTHORIZE",
                    nonce,
                    Some(json!({ "client_id": client_id, "scopes": scopes })),
                )
            })
            .await?;

        let code = payload["data"]["code"]
            .as_str()
            .ok_or(DiscordError::NoDataFound)?
            .to_owned();

        self.state = DiscordConnectionState::Authorized;
        Ok(code)
    }

    pub async fn authenticate(&mut self, token: &str) -> Result<(), DiscordError> {
        if self.state == DiscordConnectionState::NotConnected {
            return Err(DiscordError::ClientNotConnected);
        }
        self.request(|nonce| {
            PipeMessage::command(
                "AUTHENTICATE",
                nonce,
                Some(json!({ "access_token": token })),
            )
        })
        .await?;

        info!("Discord connected");
        self.state = DiscordConnectionState::Authenticated;
        Ok(())
    }

    /// Subscribes to voice settings changes.
    ///
    /// This is what removes the old 250 ms poll: Discord pushes a frame whenever the user mutes
    /// or deafens, so the app costs nothing while nothing happens.
    pub async fn subscribe_voice_settings(&self) -> Result<(), DiscordError> {
        self.require_authenticated()?;
        self.request(|nonce| {
            PipeMessage::subscription("SUBSCRIBE", VOICE_SETTINGS_UPDATE, nonce, None)
        })
        .await?;
        debug!("Subscribed to {VOICE_SETTINGS_UPDATE}");
        Ok(())
    }

    /// Reads the current voice settings.
    ///
    /// Only used once, to seed the initial state after connecting: the response carries the
    /// machine's whole audio device list, so polling it was needlessly expensive.
    pub async fn get_voice_settings(&self) -> Result<(bool, bool), DiscordError> {
        self.require_authenticated()?;
        let payload = self
            .request(|nonce| PipeMessage::command("GET_VOICE_SETTINGS", nonce, None))
            .await?;
        parse_voice_settings(&payload["data"])
    }

    pub async fn set_voice_settings(&self, mute: bool, deafen: bool) -> Result<(), DiscordError> {
        self.require_authenticated()?;
        self.request(|nonce| {
            PipeMessage::command(
                "SET_VOICE_SETTINGS",
                nonce,
                Some(json!({ "mute": mute, "deaf": deafen })),
            )
        })
        .await?;
        Ok(())
    }

    pub async fn select_voice_channel(
        &self,
        channel_id: Option<String>,
    ) -> Result<(), DiscordError> {
        self.require_authenticated()?;
        self.request(|nonce| {
            PipeMessage::command(
                "SELECT_VOICE_CHANNEL",
                nonce,
                Some(json!({ "channel_id": channel_id })),
            )
        })
        .await?;
        Ok(())
    }

    fn require_authenticated(&self) -> Result<(), DiscordError> {
        if self.state != DiscordConnectionState::Authenticated {
            return Err(DiscordError::ClientNotConnected);
        }
        Ok(())
    }
}

/// Reads `mute` and `deaf` out of a voice settings payload.
pub fn parse_voice_settings(data: &Value) -> Result<(bool, bool), DiscordError> {
    let mute = data["mute"].as_bool().ok_or(DiscordError::NoDataFound)?;
    let deafen = data["deaf"].as_bool().ok_or(DiscordError::NoDataFound)?;
    Ok((mute, deafen))
}

#[cfg(windows)]
async fn open_pipe() -> Result<Transport, DiscordError> {
    for i in 0..10 {
        if let Ok(pipe) = ClientOptions::new().open(format!(r"\\?\pipe\discord-ipc-{i}")) {
            return Ok(pipe);
        }
    }
    Err(DiscordError::PipeConnectionFailed)
}

#[cfg(unix)]
async fn open_pipe() -> Result<Transport, DiscordError> {
    let base = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
        .iter()
        .find_map(|key| var(key).ok())
        .unwrap_or_else(|| "/tmp".to_owned());

    for i in 0..10 {
        let path = std::path::Path::new(&base).join(format!("discord-ipc-{i}"));
        if let Ok(stream) = UnixStream::connect(&path).await {
            return Ok(stream);
        }
    }
    Err(DiscordError::PipeConnectionFailed)
}

/// The single reader. Every frame Discord sends passes through here.
///
/// It awaits the pipe rather than polling it, so an idle connection costs no CPU at all.
async fn read_loop(
    mut reader: ReadHalf<Transport>,
    writer: Arc<Mutex<WriteHalf<Transport>>>,
    pending: PendingReplies,
    events: mpsc::UnboundedSender<IpcEvent>,
) {
    loop {
        let message = match read_frame(&mut reader).await {
            Ok(message) => message,
            Err(err) => {
                debug!("Discord pipe closed: {err}");
                break;
            }
        };

        match message.opcode {
            Opcode::Ping => {
                // Discord expects a pong; staying silent gets the connection dropped.
                if let Ok(pong) = PipeMessage::pong()
                    && let Err(err) = writer.lock().await.write_all(&pong.to_buff()).await
                {
                    debug!("Could not answer Discord's ping: {err}");
                    break;
                }
                continue;
            }
            Opcode::Close => {
                debug!("Discord asked to close the connection");
                break;
            }
            Opcode::Pong => continue,
            _ => {}
        }

        match message.classify() {
            Ok(ResponseKind::Response { nonce, payload }) => {
                if let Some(waiter) = pending.lock().await.remove(&nonce) {
                    // The receiver is gone when the command already timed out; nothing to do.
                    let _ = waiter.send(payload);
                } else {
                    debug!("Reply for an unknown nonce, ignoring");
                }
            }
            Ok(ResponseKind::Event { event, payload }) => {
                if event == VOICE_SETTINGS_UPDATE {
                    match parse_voice_settings(&payload["data"]) {
                        Ok((mute, deafen)) => {
                            if events
                                .send(IpcEvent::VoiceSettings { mute, deafen })
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(err) => warn!("Malformed {VOICE_SETTINGS_UPDATE} payload: {err}"),
                    }
                }
            }
            Ok(ResponseKind::Unsolicited(_)) => {}
            Err(err) => warn!("Could not parse a frame from Discord: {err}"),
        }
    }

    // Whatever ended the loop, the connection is unusable. Clearing the waiters stops pending
    // commands from blocking until their timeout expires.
    pending.lock().await.clear();
    let _ = events.send(IpcEvent::Disconnected);
}

async fn read_frame(reader: &mut ReadHalf<Transport>) -> Result<PipeMessage, DiscordError> {
    let mut header = [0u8; HEADER_LEN];
    reader.read_exact(&mut header).await?;
    let (opcode, length) = PipeMessage::parse_header(header)?;

    let mut payload = vec![0u8; length as usize];
    reader.read_exact(&mut payload).await?;

    Ok(PipeMessage::new(
        opcode,
        String::from_utf8_lossy(&payload).into_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_settings_are_read_out_of_a_payload() {
        let data = json!({ "mute": true, "deaf": false, "input": { "volume": 100.0 } });
        assert_eq!(parse_voice_settings(&data).expect("parses"), (true, false));
    }

    #[test]
    fn a_payload_missing_the_flags_is_an_error_not_a_default() {
        // Silently defaulting to false here would light the wrong state on the hardware.
        assert!(parse_voice_settings(&json!({ "deaf": true })).is_err());
        assert!(parse_voice_settings(&json!({})).is_err());
    }

    #[tokio::test]
    async fn commands_are_refused_before_authentication() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let client = IpcClient::new(tx);

        assert!(matches!(
            client.get_voice_settings().await,
            Err(DiscordError::ClientNotConnected)
        ));
        assert!(matches!(
            client.set_voice_settings(true, false).await,
            Err(DiscordError::ClientNotConnected)
        ));
    }

    #[tokio::test]
    async fn a_fresh_client_reports_no_connection() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let client = IpcClient::new(tx);

        assert!(!client.is_connected());
        assert_eq!(client.state, DiscordConnectionState::NotConnected);
    }
}
