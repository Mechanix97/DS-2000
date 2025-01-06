#[derive(Debug, Clone)]
pub enum DiscordErrors {
    PipeConnectionFailed,
    PipeNotConnected,
    PipeErrorReading,
    HandshakeFailed
}