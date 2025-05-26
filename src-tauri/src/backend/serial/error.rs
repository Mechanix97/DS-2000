#[derive(Debug, Clone)]
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
}

#[derive(Debug, Clone)]

pub enum SerialMessageError {
    MalformedData,
}
