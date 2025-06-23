use crate::backend::serial::error::*;
use crate::backend::serial::serial_message::SerialMessageCodec;
use serial2_tokio::SerialPort;
use std::path::PathBuf;
use tokio::time::{timeout, Duration};
use tracing::info;

use super::messages::ping::PingMessage;
use super::serial_message::SerialMessage;

use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::codec::Framed;

pub struct Port {
    name: Option<PathBuf>,
    baudrate: u32,
    timeout: Duration,
    framed: Option<Arc<Mutex<Framed<SerialPort, SerialMessageCodec>>>>,
}

impl Port {
    pub fn new() -> Port {
        Port {
            name: None,
            baudrate: 0,
            timeout: Duration::from_millis(0),
            framed: None,
        }
    }

    pub fn connect(
        &mut self,
        port_name: PathBuf,
        baudrate: u32,
        timeout: Duration,
    ) -> Result<(), SerialPortError> {
        if self.is_connected() {
            return Err(SerialPortError::PortAlreadyConnected);
        }
        match SerialPort::open(port_name.clone(), baudrate) {
            Ok(p) => {
                eprintln!("Se conectó al puerto: {:?}", port_name);

                p.set_dtr(true).unwrap();
                p.set_rts(true).unwrap();

                let framed = Framed::new(p, SerialMessageCodec);
                self.framed = Some(Arc::new(Mutex::new(framed)));

                self.name = Some(port_name);
                self.baudrate = baudrate;
                self.timeout = timeout;

                Ok(())
            }
            Err(_) => Err(SerialPortError::PortNotAvailable),
        }
    }

    pub fn disconnect(&mut self) -> Result<(), SerialPortError> {
        self.name = None;
        self.baudrate = 0;
        self.timeout = Duration::from_millis(0);
        self.framed = None;
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
            eprintln!("Trying to connect to port {:?}", p);
            match self.connect(p, baudrate, timeout) {
                Ok(_) => match self.authenticate().await {
                    Ok(_) => {
                        return Ok(());
                    }
                    Err(e) => {
                        info!("Disconnecting");
                        self.disconnect().map_err(|_| e)?; // Si falla el disconnect, devolvemos el error original
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

    pub async fn authenticate(&mut self) -> Result<(), SerialPortError> {
        eprintln!("WRITE");
        self.send_message(&SerialMessage::Ping(PingMessage {}))
            .await?;
        eprintln!("WRITE done");

        let timeout_duration = Duration::from_secs(1);
        let msg = timeout(timeout_duration, self.read_message())
            .await
            .map_err(|_| SerialPortError::TimedOut)??;

        eprintln!("READ");
        match msg {
            SerialMessage::Pong(_) => {
                info!("Authentication successful");
                Ok(())
            }
            _ => {
                info!("Authentication failed");
                Err(SerialPortError::AuthenticationFailed)
            }
        }
    }

    pub fn is_connected(&self) -> bool {
        self.framed.is_some()
    }

    pub async fn send_message(&self, msg: &SerialMessage) -> Result<(), SerialPortError> {
        if let Some(framed_mutex) = &self.framed {
            let mut framed = framed_mutex.lock().await;
            framed.send(msg.clone()).await?;
            Ok(())
        } else {
            Err(SerialPortError::PortNotConnected)
        }
    }

    pub async fn read_message(&self) -> Result<SerialMessage, SerialPortError> {
        if let Some(framed_mutex) = &self.framed {
            let mut framed = framed_mutex.lock().await;
            match framed.next().await {
                Some(Ok(msg)) => Ok(msg),
                Some(Err(e)) => Err(e),
                None => Err(SerialPortError::PortNotConnected),
            }
        } else {
            Err(SerialPortError::PortNotConnected)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::backend::serial::messages::pong::PongMessage;

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

    #[tokio::test]
    pub async fn test_double_ping() {
        let mut port = Port::new();
        match port.auto_connect(115200, Duration::from_micros(1000)).await {
            Err(e) => eprintln!("Error: {:?}", e),
            Ok(_) => {}
        }

        assert!(port.is_connected());

        port.send_message(&SerialMessage::Ping(PingMessage {}))
            .await
            .unwrap();

        let pong = port.read_message().await.unwrap();

        assert_eq!(pong, SerialMessage::Pong(PongMessage {}));

        port.disconnect().unwrap();
    }
}
