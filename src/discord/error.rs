#[derive(Debug, Clone)]
pub enum DiscordError {
    PipeConnectionFailed,
    PipeNotConnected,
    PipeErrorReading,
    PipeWriteError,
    HandshakeFailed,
    ClientIdNotFound
}