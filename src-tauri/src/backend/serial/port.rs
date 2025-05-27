use crate::backend::serial::error::*;
use serialport::SerialPort;
use std::io::BufRead;
use std::io::BufReader;
use std::time::Duration;
use std::time::Instant;
use tracing::info;

use super::ping::PingMessage;
use super::serial_message::SerialMessage;

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
                        info!("Se conecto al puerto: {}", port_name);
                        self.name = Some(String::from(port_name));
                        self.baudrate = baudrate;
                        self.timeout = timeout;
                        self.port = Some(p);
                        Ok(())
                    }
                    Err(_) => Err(SerialPortError::PortNotAvailable),
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
            info!("Trying to connect to port {}", p);
            match self.connect(p.as_str(), baudrate, timeout) {
                Ok(_) => match self.authenticate() {
                    Ok(_) => {
                        return Ok(());
                    }
                    Err(e) => {
                        info!("Desconnecting");
                        self.disconnect().map_err(|_| e)?;
                        continue;
                    }
                },
                Err(e) => {
                    info!("{:?}", e);
                }
            }
        }
        Err(SerialPortError::PortNotConnected)
    }

    //Metodo para autenticarse con el puerto serie. Habria que ver como se complicarlo para que no se pueda acceder al programa con un dispositivo no autorizado
    pub fn authenticate(&mut self) -> Result<(), SerialPortError> {
        self.send_message(&SerialMessage::Ping(PingMessage {}))?;
        let msg = self.read_message()?;

        match msg {
            SerialMessage::Pong(_) => {
                info!("Authentication succesful");
                Ok(())
            }
            _ => {
                info!("Autentication failed");
                Err(SerialPortError::AuthenticationFailed)
            }
        }

        // match &mut self.port {
        //     Some(p) => {
        //         // Enviar PING\n
        //         p.as_mut()
        //             .write(b"PING\n")
        //             .map_err(|_| SerialPortError::PortNotConnected)?;
        //         p.flush().map_err(|_| SerialPortError::PortNotConnected)?; // Asegurar que se envíe

        //         // Buffer para leer
        //         let mut buf = [0u8; 64];
        //         let mut total_bytes = 0;
        //         let expected_bytes = 5; // "PONG\n" o "pong\n" tiene 5 bytes
        //         let timeout = Duration::from_millis(1000); // Timeout total de 1 segundo
        //         let start = Instant::now();

        //         // Leer hasta obtener los bytes esperados o timeout
        //         while total_bytes < expected_bytes && start.elapsed() < timeout {
        //             match p.read(&mut buf[total_bytes..]) {
        //                 Ok(n) => {
        //                     total_bytes += n;
        //                 }
        //                 Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
        //                     // Continuar si hay timeout parcial
        //                     continue;
        //                 }
        //                 Err(_) => return Err(SerialPortError::PortNotConnected),
        //             }
        //         }

        //         // Convertir a string
        //         let response = String::from_utf8_lossy(&buf[..total_bytes]).to_string();

        //         // Verificar respuesta (ajusta según el firmware: "PONG\n" o "pong\n")
        //         if response != "PONG\r\n" {
        //             // Cambia a "pong\n" si modificaste el firmware
        //             return Err(SerialPortError::AuthenticationFailed);
        //         }

        //         info!("Autenticación exitosa");
        //         Ok(())
        //     }
        //     None => Err(SerialPortError::PortNotConnected),
        // }
    }

    pub fn is_connected(&self) -> bool {
        match self.port {
            Some(_) => true,
            None => false,
        }
    }

    pub fn send_message(&mut self, msg: &SerialMessage) -> Result<(), SerialPortError> {
        let mut buf = vec![];
        msg.encode(&mut buf)
            .map_err(|e| SerialPortError::ErrorEncodingMsg(e))?;
        match &mut self.port {
            Some(p) => {
                p.as_mut()
                    .write(&buf)
                    .map_err(|_| SerialPortError::PortNotConnected)?;
                p.flush().map_err(|_| SerialPortError::PortNotConnected)?;

                Ok(())
            }
            None => Err(SerialPortError::PortNotConnected),
        }
    }

    pub fn read_message(&mut self) -> Result<SerialMessage, SerialPortError> {
        match &mut self.port {
            Some(p) => {
                let mut buf = Vec::new();
                let timeout = Duration::from_millis(1000); // 1-second timeout
                let start = Instant::now();

                // Read one byte at a time until 0xFF or timeout
                while start.elapsed() < timeout {
                    let mut byte = [0u8; 1];
                    match p.read(&mut byte) {
                        Ok(1) => {
                            buf.push(byte[0]);
                            eprintln!("Read byte: 0x{:02X}, buffer: {:?}", byte[0], buf);
                            if byte[0] == 0xFF {
                                break; // Stop reading when delimiter 0xFF is found
                            }
                        }
                        Ok(n) => {
                            info!("Unexpected read count: {}", n);
                            return Err(SerialPortError::ErrorReadingPort);
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                            info!("Timeout while reading byte");
                            continue;
                        }
                        Err(_) => return Err(SerialPortError::ErrorReadingPort),
                    }
                }

                if buf.is_empty() || buf[buf.len() - 1] != 0xFF {
                    info!("Failed to read complete message with 0xFF delimiter");
                    return Err(SerialPortError::ErrorReadingPort);
                }

                // Decode the buffer
                let msg = SerialMessage::decode(&buf)
                    .map_err(|e| SerialPortError::ErrorDecodingMsg(e))?;
                info!("Decoded message: {:?}", msg);
                Ok(msg)
            }
            None => Err(SerialPortError::PortNotConnected),
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    pub fn test_auto_connect() {
        let mut port = Port::new();
        match port.auto_connect(115200, Duration::from_micros(1000)) {
            Err(e) => eprintln!("Error: {:?}", e),
            Ok(_) => {}
        }

        assert!(port.is_connected());
    }
}
