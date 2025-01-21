use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;
use std::sync::{Arc, Mutex};

use crate::serial::error::*;
use crate::serial::port::*;

pub enum SerialWorkerMessage{
    Stop
}

#[derive(Clone, Copy)]
pub enum SerialWorkerStatus{
    PortConnected,
    PortNotConnected,
    Stopped
}

pub struct SerialWorker{
    //port: Port,
    status: Arc<Mutex<SerialWorkerStatus>>,
    thread: Option<thread::JoinHandle<()>>,
    tx: Option<Sender<SerialWorkerMessage>>,
    rx: Option<Receiver<SerialWorkerMessage>>
}


impl SerialWorker {
    pub fn new() -> SerialWorker{
        SerialWorker{
           //port: Port::new(),
           status: Arc::new(Mutex::new(SerialWorkerStatus::PortNotConnected)),
           thread: None,
           tx: None,
           rx: None
        }
    }


    pub fn start(&mut self, port_name: Option<String>) -> Result<(), SerialPortError>{
        let (tx, rx_thread) = mpsc::channel();
        let (tx_thread, rx) = mpsc::channel(); 
        self.tx = Some(tx);
        self.rx = Some(rx);
        let mut st = self.status.clone();
        let t = thread::spawn(move || {
            // let val = String::from("Hola");
            // tx.send(val).unwrap();

            //inicializo el puerto serie e intento conectarlo al com guardado en la configuracion
            let mut port = Port::new();
            match port_name {
                Some(p) => {
                    match port.connect(p.as_str(), 9600, Duration::from_millis(100)) {
                        Ok(_) => {
                            *st.lock().unwrap() = SerialWorkerStatus::PortConnected;
                        }
                        Err(_e) => { //No se pudo conectar al puerto
                            *st.lock().unwrap() = SerialWorkerStatus::PortNotConnected;
                        }
                    }                    
                }
                None => {} //Si no hay paramatro, entonces no habia configuracion guardada
            }

            loop{
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
                    }
                    SerialWorkerStatus::Stopped=>{
                        break;
                    }
                }
                match rx_thread.recv_timeout(Duration::from_millis(100)){
                    Ok(msg) => {
                        match msg {
                            SerialWorkerMessage::Stop => {
                                if port.is_connected(){
                                    port.disconnect().unwrap();
                                }
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        // println!("Error: {}", e);
                    }
                }
            }
        });
        self.thread = Some(t);

        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), SerialPortError>{

        match &self.tx {
            Some(tx) => {
                tx.send(SerialWorkerMessage::Stop).unwrap();
            }
            None => {
                return Err(SerialPortError::InternalChannelClosed);
            }
        }

        if let Some(handle) = self.thread.take() {
            match handle.join(){
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

}