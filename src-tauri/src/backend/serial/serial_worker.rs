use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::info;

use super::error::SerialPortError;
use super::port::Port;

use super::serial_message::SerialMessage;

pub enum SerialWorkerMessage {
    Stop,
}

#[derive(Clone, Copy)]
pub enum SerialWorkerStatus {
    PortConnected,
    PortNotConnected,
    Stopped,
}

struct PortRecord {
    port_name: String,
    last_tried: u64,
}

pub struct SerialWorker {
    status: Arc<Mutex<SerialWorkerStatus>>,
    thread: Option<tokio::task::JoinHandle<()>>,
    tx: Option<Sender<SerialWorkerMessage>>,
    rx: Option<Receiver<SerialWorkerMessage>>,
    muted: Arc<AtomicBool>,
    deafen: Arc<AtomicBool>,
    disconnect: Arc<AtomicBool>,
}

impl SerialWorker {
    pub fn new() -> SerialWorker {
        SerialWorker {
            status: Arc::new(Mutex::new(SerialWorkerStatus::PortNotConnected)),
            thread: None,
            tx: None,
            rx: None,
            muted: Arc::new(AtomicBool::new(false)),
            deafen: Arc::new(AtomicBool::new(false)),
            disconnect: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn start(&mut self, port_name: Option<String>) -> Result<(), SerialPortError> {
        let (tx, rx_thread) = mpsc::channel();
        let (_tx_thread, rx) = mpsc::channel();
        self.tx = Some(tx);
        self.rx = Some(rx);

        let status = self.status.clone();
        let _muted = self.muted.clone();
        let deafen = self.deafen.clone();
        let disconnect = self.disconnect.clone();

        let handle = tokio::spawn(async move {
            let mut port = Port::new();
            if let Some(p) = port_name {
                match port.connect(
                    &PathBuf::from(p.clone()),
                    115200,
                    Duration::from_millis(100),
                ) {
                    Ok(_) => {
                        *status.lock().unwrap() = SerialWorkerStatus::PortConnected;
                        info!("Connected to port: {}", p);
                    }
                    Err(e) => {
                        *status.lock().unwrap() = SerialWorkerStatus::PortNotConnected;
                        info!("Failed to connect to port {}: {:?}", p, e);
                    }
                }
            }

            loop {
                let current_status = *status.lock().unwrap();
                match current_status {
                    SerialWorkerStatus::PortNotConnected => {
                        match port.auto_connect(9600, Duration::from_millis(10000)).await {
                            Ok(_) => {
                                *status.lock().unwrap() = SerialWorkerStatus::PortConnected;
                                info!("Auto-connected to port");
                            }
                            Err(e) => {
                                *status.lock().unwrap() = SerialWorkerStatus::PortNotConnected;
                                info!("Auto-connect failed: {:?}", e);
                            }
                        }
                    }
                    SerialWorkerStatus::PortConnected => {
                        if disconnect.load(Ordering::SeqCst) {
                            if let Err(e) = port.disconnect().await {
                                info!("Failed to disconnect port: {:?}", e);
                            }
                            *status.lock().unwrap() = SerialWorkerStatus::PortNotConnected;
                            continue;
                        }

                        if !deafen.load(Ordering::SeqCst) {
                            match port.read_message(Duration::from_millis(100)).await {
                                Ok(msg) => match msg {
                                    SerialMessage::Ping(_) => {
                                        info!("msg PING received");
                                    }
                                    SerialMessage::Pong(_) => {
                                        info!("msg PONG received");
                                    }
                                },
                                Err(e) => {
                                    info!("Error reading message: {:?}", e);
                                    *status.lock().unwrap() = SerialWorkerStatus::PortNotConnected;
                                }
                            }
                        }
                    }
                    SerialWorkerStatus::Stopped => {
                        if port.is_connected() {
                            if let Err(e) = port.disconnect().await {
                                info!("Failed to disconnect port: {:?}", e);
                            }
                        }
                        break;
                    }
                }

                if let Ok(msg) = rx_thread.try_recv() {
                    match msg {
                        SerialWorkerMessage::Stop => {
                            if port.is_connected() {
                                if let Err(e) = port.disconnect().await {
                                    info!("Failed to disconnect port: {:?}", e);
                                }
                            }
                            *status.lock().unwrap() = SerialWorkerStatus::Stopped;
                            break;
                        }
                    }
                }

                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });

        self.thread = Some(handle);

        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), SerialPortError> {
        match &self.tx {
            Some(tx) => {
                tx.send(SerialWorkerMessage::Stop).unwrap();
            }
            None => {
                return Err(SerialPortError::InternalChannelClosed);
            }
        }

        if let Some(handle) = self.thread.take() {
            // match handle.drop() {
            //     Ok(_) => {
            //         self.thread = None;
            //     }
            //     Err(e) => {
            //         info!("Error cerrando thread: {:?}", e);
            //         return Err(SerialPortError::ErrorClosingThread);
            //     }
            // }
        }

        Ok(())
    }

    fn parse_message(&self, line: &String) {
        match line.as_str() {
            s if s.starts_with("DSST") => {
                info!("DSST");
            }
            s if s.starts_with("HWST-") => {}
            _ => {
                println!("Comando desconocido");
            }
        }
    }

    pub fn get_voice_settings(&mut self) -> (bool, bool) {
        let m = self.muted.load(Ordering::SeqCst);
        let d = self.deafen.load(Ordering::SeqCst);

        (m, d)
    }

    pub fn get_disconenct(&mut self) -> bool {
        let m = self.disconnect.load(Ordering::SeqCst);
        self.disconnect.store(false, Ordering::SeqCst);
        m
    }

    pub fn has_update(&self) -> bool {
        false
    }
}
