use serialport::SerialPort;
use std::time::Duration;
use std::io::BufReader;
use std::{sync::{Arc, RwLock}, io::BufRead};

use crate::serial::error::*;

pub struct Port{
    name: Option<String>,
    baudrate: u32,
    timeout: Duration,
    port: Option<Box<dyn SerialPort>>
}


impl Port{
    pub fn new() -> Port{
        Port{
            name: None,
            baudrate: 0,
            timeout: Duration::from_millis(0),
            port: None
        }
    }

    pub fn connect(&mut self, port_name: &str, baudrate: u32, timeout: Duration) -> Result<(), SerialPortError>{
        match self.port {
            Some(_) => {
                return Err(SerialPortError::PortAlreadyConnected);
            }
            None => {
                match serialport::new(port_name, baudrate)
                .timeout(timeout)
                .flow_control(serialport::FlowControl::None)
                .open() {
                    Ok(p) => {
                        println!("Se conecto al puerto: {}", port_name);
                        self.name = Some(String::from(port_name));
                        self.baudrate = baudrate;
                        self.timeout = timeout;
                        self.port = Some(p);
                        return Ok(());
                    }
                    Err(e) => {
                        println!("Error conectando: {}", e);
                        return Err(SerialPortError::PortNotAvailable);
                    }
                }
            }   
        }
    }

    pub fn disconnect(&mut self) -> Result<(), SerialPortError> {
        self.name = None;
        self.baudrate = 0;
        self.timeout = Duration::from_millis(0);
        self.port = None;
        Ok(())
    }


    pub fn get_ports(&self) -> Result<Vec<String>, SerialPortError> {
        let mut ports = vec![];
        for port in serialport::available_ports().unwrap() {
            ports.push(port.port_name);
        }
        Ok(ports)
    }

    pub fn auto_connect(&mut self, baudrate: u32, timeout: Duration) -> Result<(), SerialPortError> {
        let mut  available_ports = self.get_ports()?;
        available_ports.sort();
        for p in available_ports {
            println!("Trying to connect to port {}", p);
            match self.connect(p.as_str(), baudrate, timeout) {
                Ok(_) => { 
                    match self.authenticate(){
                        Ok(_) => {
                            println!("Conectado OK: {}", p);
                            return Ok(());
                        }
                        Err(e) => {
                            println!("No se pudo autenticar, desconecto {}", p);
                            self.disconnect();
                            continue;
                        }
                    }
                },
                Err(e) => {
                    println!("{:?}", e);
                }
            }
        }
        Err(SerialPortError::PortNotConnected)
    }

    //Metodo para autenticarse con el puerto serie. Habria que ver como se complicarlo para que no se pueda acceder al programa con un dispositivo no autorizado
    pub fn authenticate(&mut self) -> Result<(), SerialPortError>{
        match &mut self.port {
            Some(p) => {
                p.as_mut().write(b"PING\n");
                
                let mut buf = Vec::new();
                p.read_to_end(&mut buf);
                let response = match String::from_utf8(buf) {
                    Ok(s) => s,
                    Err(e) => {
                        println!("Error de codificación UTF-8: {}", e);
                        return Err(SerialPortError::PortNotConnected);
                    }
                };
                if response != "PONG\r\n"{ 
                    return Err(SerialPortError::PortNotConnected);
                }
                
                Ok(())
            }
            None => {
                Err(SerialPortError::PortNotConnected)
            }
        }
    }

    pub fn is_connected(&self) -> bool {
        match self.port {
            Some(_) => true,
            None => false
        }
    }

    pub fn read_line(&mut self) -> Result<String, SerialPortError> {
        match &mut self.port {
            Some(p) => {
                let mut reader = BufReader::new(p.try_clone().unwrap());
                let mut my_str = String::new();
                match reader.read_line(&mut my_str) {
                    Ok(_) => {
                        return Ok(my_str);
                    }
                    Err(e) => {
                        if e.kind() != std::io::ErrorKind::TimedOut {
                            println!("Otro error: {}", e.kind());
                        }
                        return Err(SerialPortError::TimedOut);
                    }
                }               
            }
            None => {
                Err(SerialPortError::PortNotConnected)
            }
        }
    }
}