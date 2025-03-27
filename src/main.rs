pub mod config;
pub mod discord;
pub mod serial;

use std::{thread, time};

// use core::time;
use std::time::Duration;

use discord::worker::DiscordWorker;
// use serial::port::Port;
// use serial::worker::SerialWorker;
use config::config::Config;
use std::io::{self};

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#[cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    app_lib::run();
}
