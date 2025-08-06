use spawned_concurrency::error::GenServerError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SerialPortError {
    #[error("Port is not available")]
    PortNotAvailable,
    #[error("Port is already connected")]
    PortAlreadyConnected,
    #[error("Port is not connected")]
    PortNotConnected,
    #[error("Internal channel closed unexpectedly")]
    InternalChannelClosed,
    #[error("Error closing thread")]
    ErrorClosingThread,
    #[error("Operation timed out")]
    TimedOut,
    #[error("Authentication failed")]
    AuthenticationFailed,
    #[error("Error reading from port")]
    ErrorReadingPort,
    #[error("Message encoding error: {0}")]
    ErrorEncodingMsg(SerialMessageError),
    #[error("Message decoding error: {0}")]
    ErrorDecodingMsg(SerialMessageError),
    #[error("Internal error occurred")]
    InternalError,
    #[error("I/O error: {0}")]
    IoError(std::io::Error),

    #[error("Spawned GenServer Error")]
    GenServerError(GenServerError),
}

impl From<std::io::Error> for SerialPortError {
    fn from(err: std::io::Error) -> Self {
        SerialPortError::IoError(err)
    }
}

impl From<SerialMessageError> for SerialPortError {
    fn from(err: SerialMessageError) -> Self {
        SerialPortError::ErrorDecodingMsg(err)
    }
}

#[derive(Error, Debug, Clone)]

pub enum SerialMessageError {
    #[error("Error malformed data")]
    MalformedData,
}
