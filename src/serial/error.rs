#[derive(Debug, Clone)]
pub enum SerialPortError {
    PortNotAvailable,
    PortAlreadyConnected,
    PortNotConnected
}