use crate::backend::serial::error::*;
use serial2_tokio::SerialPort;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use tracing::info;

use super::messages::ping::PingMessage;
use super::serial_message::SerialMessage;

pub struct Port {
    name: Option<PathBuf>,
    baudrate: u32,
    timeout: Duration,
    port: Option<SerialPort>,
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
        port_name: PathBuf,
        baudrate: u32,
        timeout: Duration,
    ) -> Result<(), SerialPortError> {
        match self.port {
            Some(_) => {
                return Err(SerialPortError::PortAlreadyConnected);
            }
            None => match SerialPort::open(port_name.clone(), baudrate) {
                Ok(p) => {
                    info!("Se conecto al puerto: {:?}", port_name);
                    p.set_dtr(true).unwrap();
                    p.set_rts(true).unwrap();
                    self.name = Some(port_name);
                    self.baudrate = baudrate;
                    self.timeout = timeout;
                    self.port = Some(p);
                    Ok(())
                }
                Err(_) => Err(SerialPortError::PortNotAvailable),
            },
        }
    }

    pub fn disconnect(&mut self) -> Result<(), SerialPortError> {
        self.name = None;
        self.baudrate = 0;
        self.timeout = Duration::from_millis(0);
        self.port = None;
        Ok(())
    }

    pub fn get_ports(&self) -> Result<Vec<PathBuf>, SerialPortError> {
        let mut ports = vec![];
        for port in SerialPort::available_ports().unwrap() {
            ports.push(port);
        }
        Ok(ports)
    }

    pub async fn auto_connect(
        &mut self,
        baudrate: u32,
        timeout: Duration,
    ) -> Result<(), SerialPortError> {
        let mut available_ports = self.get_ports()?;
        available_ports.sort();
        for p in available_ports {
            info!("Trying to connect to port {:?}", p);
            match self.connect(p, baudrate, timeout) {
                Ok(_) => match self.authenticate().await {
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
    pub async fn authenticate(&mut self) -> Result<(), SerialPortError> {
        self.send_message(&SerialMessage::Ping(PingMessage {}))
            .await?;
        let msg = self.read_message().await?;
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
    }

    pub fn is_connected(&self) -> bool {
        match self.port {
            Some(_) => true,
            None => false,
        }
    }

    pub async fn send_message(&mut self, msg: &SerialMessage) -> Result<(), SerialPortError> {
        let mut buf = vec![];
        msg.encode(&mut buf)
            .map_err(|e| SerialPortError::ErrorEncodingMsg(e))?;
        match &mut self.port {
            Some(p) => {
                p.write(&buf).await.unwrap();

                Ok(())
            }
            None => Err(SerialPortError::PortNotConnected),
        }
    }

    pub async fn read_message(&mut self) -> Result<SerialMessage, SerialPortError> {
        match &mut self.port {
            Some(p) => {
                let mut buf = Vec::new();
                let timeout = Duration::from_millis(1000); // 1-second timeout
                let start = Instant::now();

                // Read one byte at a time until 0xFF or timeout
                while start.elapsed() < timeout {
                    let mut byte = [0u8; 1];
                    match p.read(&mut byte).await {
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

    #[tokio::test]
    pub async fn test_auto_connect() {
        let mut port = Port::new();
        match port.auto_connect(115200, Duration::from_micros(1000)).await {
            Err(e) => eprintln!("Error: {:?}", e),
            Ok(_) => {}
        }

        assert!(port.is_connected());

        port.disconnect().unwrap();
    }


}
