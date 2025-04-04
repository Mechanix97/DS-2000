pub mod client;
pub mod error;
pub mod pipemessage;
pub mod utils;
pub mod worker;

#[cfg(unix)]
pub mod ipc_unix;

#[cfg(windows)]
pub mod ipc_windows;
