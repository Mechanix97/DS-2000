use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use std::sync::atomic::{AtomicBool, Ordering};

use crate::backend::serial::error::*;
use crate::backend::serial::port::*;

pub enum SerialWorkerMessage {
    Stop,
}


#[derive(Clone, Copy)]
pub enum SerialWorkerStatus {
    PortConnected,
    PortNotConnected,
    Stopped,
}

pub struct SerialWorker {
    status: Arc<Mutex<SerialWorkerStatus>>,
    thread: Option<thread::JoinHandle<()>>,
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

    pub fn start(&mut self, port_name: Option<String>) -> Result<(), SerialPortError> {
        let (tx, rx_thread) = mpsc::channel();
        let (_tx_thread, rx) = mpsc::channel();
        self.tx = Some(tx);
        self.rx = Some(rx);

        let st = self.status.clone();
        let muted = self.muted.clone();
        let deafen = self.deafen.clone();
        let disconnect = self.disconnect.clone();

        let t = thread::spawn(move || {
            //inicializo el puerto serie e intento conectarlo al com guardado en la configuracion
            let mut port = Port::new();
            match port_name {
                Some(p) => {
                    match port.connect(p.as_str(), 9600, Duration::from_millis(100)) {
                        Ok(_) => {
                            *st.lock().unwrap() = SerialWorkerStatus::PortConnected;
                        }
                        Err(_e) => {
                            //No se pudo conectar al puerto
                            *st.lock().unwrap() = SerialWorkerStatus::PortNotConnected;
                        }
                    }
                }
                None => {} //Si no hay paramatro, entonces no habia configuracion guardada
            }

            loop {
                let current_status;
                {
                    current_status = *st.lock().unwrap();
                }

                match current_status {
                    SerialWorkerStatus::PortNotConnected => {
                        match port.auto_connect(9600, Duration::from_millis(100)) {
                            Ok(_) => {
                                *st.lock().unwrap() = SerialWorkerStatus::PortConnected;
                            }
                            Err(_) => {
                                *st.lock().unwrap() = SerialWorkerStatus::PortNotConnected;
                            }
                        }
                    }
                    SerialWorkerStatus::PortConnected => {
                        //logica de msg con la placa
                        match port.read_line() {
                            Ok(line) => {
                                match line.as_str() {
                                    s if s.starts_with("DSST") => {
                                        println!("DSST");
                                    }
                                    s if s.starts_with("HWST-") => {
                                        let parts: Vec<&str> = s.split('-').collect();

                                        // Obtener la última parte que contiene los números
                                        if let Some(last_part) = parts.last() {
                                            let digit1 = last_part
                                                .chars()
                                                .nth(0)
                                                .unwrap()
                                                .to_digit(10)
                                                .unwrap();
                                            let digit2 = last_part
                                                .chars()
                                                .nth(1)
                                                .unwrap()
                                                .to_digit(10)
                                                .unwrap();
                                            let digit3 = last_part
                                                .chars()
                                                .nth(2)
                                                .unwrap()
                                                .to_digit(10)
                                                .unwrap();
                                            let mute_b = digit1 == 1;
                                            let deaf_b = digit2 == 1;
                                            let disconnect_b = digit3 == 1;

                                            muted.store(mute_b, Ordering::SeqCst);
                                            deafen.store(deaf_b, Ordering::SeqCst);
                                            disconnect.store(disconnect_b, Ordering::SeqCst);
                                        }
                                    }
                                    _ => {
                                        println!("Comando desconocido");
                                    }
                                }
                            }
                            Err(_e) => {}
                        }
                    }
                    SerialWorkerStatus::Stopped => {
                        break;
                    }
                }
                match rx_thread.recv_timeout(Duration::from_millis(10)) {
                    Ok(msg) => match msg {
                        SerialWorkerMessage::Stop => {
                            if port.is_connected() {
                                port.disconnect().unwrap();
                            }
                            break;
                        }
                    },
                    Err(_) => {}
                }
            }
        });
        self.thread = Some(t);

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
            match handle.join() {
                Ok(_) => {
                    self.thread = None;
                }
                Err(e) => {
                    println!("Error cerrando thread: {:?}", e);
                    return Err(SerialPortError::ErrorClosingThread);
                }
            }
        }

        Ok(())
    }

    fn parse_message(&self, line: &String) {
        match line.as_str() {
            s if s.starts_with("DSST") => {
                println!("DSST");
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
}
