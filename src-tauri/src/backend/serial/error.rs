#[derive(Debug)]
pub enum SerialPortError {
    PortNotAvailable,
    PortAlreadyConnected,
    PortNotConnected,
    InternalChannelClosed,
    ErrorClosingThread,
    TimedOut,
    AuthenticationFailed,
    ErrorReadingPort,
    ErrorEncodingMsg(SerialMessageError),
    ErrorDecodingMsg(SerialMessageError),
    InternalError,
    IoError(std::io::Error),
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

#[derive(Debug, Clone)]

pub enum SerialMessageError {
    MalformedData,
}
