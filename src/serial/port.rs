use serialport::SerialPort;
use std::time::Duration;

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
                        println!("Se cpenctp {}", port_name);
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
            match self.connect(p.as_str(), baudrate, timeout) {
                Ok(_) => { 
                    println!("Conectado");
                    return Ok(());
                },
                Err(e) => {
                    println!("{:?}", e);
                }
            }
        }

        Ok(())
    }
}