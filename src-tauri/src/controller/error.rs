use thiserror::Error;

use discord::error::DiscordError;
use serial::error::SerialPortError;
use std::time::SystemTimeError;

#[derive(Error, Debug)]
pub enum ControllerError {
    #[error("Generic error: {0}")]
    GenericError(String),

    #[error("Error in discord interface: {0}")]
    DiscordError(#[from] DiscordError),

    #[error("Error in serial interface: {0}")]
    SerialPortError(#[from] SerialPortError),

    #[error("Tauri error: {0}")]
    Tauri(#[from] tauri::Error),

    #[error("Error in system time: {0}")]
    SystemTimeError(#[from] SystemTimeError),
}
