use thiserror::Error;

#[derive(Error, Debug)]
pub enum DiscordError {
    #[error("Failed to connect to the Discord pipe")]
    PipeConnectionFailed,

    #[error("Discord pipe is not connected")]
    PipeNotConnected,

    #[error("Error reading from the Discord pipe")]
    PipeErrorReading,

    #[error("Error writing to the Discord pipe")]
    PipeWriteError,

    #[error("Discord handshake failed")]
    HandshakeFailed,

    #[error("Client ID not found")]
    ClientIdNotFound,

    #[error("Failed to convert data with Serde: {0}")]
    SerdeConvertionError(#[from] serde_json::Error),

    #[error("Authorization with Discord failed")]
    AuthorizationFailed,

    #[error("Authentication with Discord failed")]
    AuthenticationFailed,

    #[error("No data found in the response")]
    NoDataFound,

    #[error("Internal communication channel is closed")]
    InternalChannelClosed,

    #[error("Error closing the thread")]
    ErrorClosingThread,

    #[error("Pipe read error: {0}")]
    PipeReadError(#[from] std::io::Error),

    #[error("Handshake not performed")]
    HandshakeNotDone,
}
