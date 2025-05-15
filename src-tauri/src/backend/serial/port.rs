use serialport::SerialPort;
use std::io::BufRead;
use std::io::BufReader;
use std::time::Duration;
use std::time::Instant;

use crate::backend::serial::error::*;

pub struct Port {
    name: Option<String>,
    baudrate: u32,
    timeout: Duration,
    port: Option<Box<dyn SerialPort>>,
}

impl Port {
    pub fn new() -> Port {
        Port {
            name: None,
            baudrate: 0,
            timeout: Duration::from_millis(0),
            port: None,
        }
    }

    pub fn connect(
        &mut self,
        port_name: &str,
        baudrate: u32,
        timeout: Duration,
    ) -> Result<(), SerialPortError> {
        match self.port {
            Some(_) => {
                return Err(SerialPortError::PortAlreadyConnected);
            }
            None => {
                match serialport::new(port_name, baudrate)
                    .timeout(timeout)
                    .flow_control(serialport::FlowControl::None)
                    .open()
                {
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

    pub fn auto_connect(
        &mut self,
        baudrate: u32,
        timeout: Duration,
    ) -> Result<(), SerialPortError> {
        let mut available_ports = self.get_ports()?;
        available_ports.sort();
        for p in available_ports {
            println!("Trying to connect to port {}", p);
            match self.connect(p.as_str(), baudrate, timeout) {
                Ok(_) => match self.authenticate() {
                    Ok(_) => {
                        println!("Conectado OK: {}", p);
                        return Ok(());
                    }
                    Err(e) => {
                        println!("No se pudo autenticar, desconecto {}", p);
                        self.disconnect().map_err(|_| e)?;
                        continue;
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

pub fn authenticate(&mut self) -> Result<(), SerialPortError> {
    match &mut self.port {
        Some(p) => {
            println!("hago ping");
            // Enviar PING\n
            p.as_mut()
                .write(b"PING\n")
                .map_err(|_| SerialPortError::PortNotConnected)?;
            p.flush().map_err(|_| SerialPortError::PortNotConnected)?; // Asegurar que se envíe

            // Buffer para leer
            let mut buf = [0u8; 64];
            let mut total_bytes = 0;
            let expected_bytes = 5; // "PONG\n" o "pong\n" tiene 5 bytes
            let timeout = Duration::from_millis(1000); // Timeout total de 1 segundo
            let start = Instant::now();

            // Leer hasta obtener los bytes esperados o timeout
            while total_bytes < expected_bytes && start.elapsed() < timeout {
                match p.read(&mut buf[total_bytes..]) {
                    Ok(n) => {
                        total_bytes += n;
                        println!("Bytes parciales recibidos: {:?}", &buf[..total_bytes]);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                        // Continuar si hay timeout parcial
                        continue;
                    }
                    Err(_) => return Err(SerialPortError::PortNotConnected),
                }
            }

            // Convertir a string
            let response = String::from_utf8_lossy(&buf[..total_bytes]).to_string();
            println!("Respuesta completa: |{}|", response);
            println!("Bytes totales recibidos: {:?}", &buf[..total_bytes]);

            // Verificar respuesta (ajusta según el firmware: "PONG\n" o "pong\n")
            if response != "PONG\r\n" { // Cambia a "pong\n" si modificaste el firmware
                println!("Respuesta inesperada: |{}|", response);
                return Err(SerialPortError::AuthenticationFailed);
            }

            println!("Autenticación exitosa");
            Ok(())
        }
        None => Err(SerialPortError::PortNotConnected),
    }
}
    pub fn is_connected(&self) -> bool {
        match self.port {
            Some(_) => true,
            None => false,
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
            None => Err(SerialPortError::PortNotConnected),
        }
    }
}
