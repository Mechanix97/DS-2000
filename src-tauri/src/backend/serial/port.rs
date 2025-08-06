use super::error::SerialPortError;
use super::messages::ping::PingMessage;
use super::serial_message::SerialMessage;
use super::serial_message::SerialMessageCodec;

use futures_util::{SinkExt, StreamExt};
use serial2_tokio::SerialPort;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, timeout};
use tokio_util::codec::Framed;
use tracing::{debug, info};

#[derive(Clone)]
pub struct Port {
    name: Option<String>,
    baudrate: u32,
    timeout: Duration,
    framed: Option<Arc<Mutex<Framed<SerialPort, SerialMessageCodec>>>>,
    connected: bool,
}

impl Port {
    pub fn new() -> Port {
        Port {
            name: None,
            baudrate: 0,
            timeout: Duration::from_millis(0),
            framed: None,
            connected: false,
        }
    }

    pub fn connect(
        &mut self,
        port_name: &PathBuf,
        baudrate: u32,
        timeout: Duration,
    ) -> Result<(), SerialPortError> {
        if self.is_connected() {
            return Err(SerialPortError::PortAlreadyConnected);
        }
        match SerialPort::open(port_name.clone(), baudrate) {
            Ok(p) => {
                p.set_dtr(true).unwrap();
                p.set_rts(true).unwrap();

                let framed = Framed::new(p, SerialMessageCodec);
                self.framed = Some(Arc::new(Mutex::new(framed)));
                self.name = port_name.to_str().map(|s| s.to_string());
                self.baudrate = baudrate;
                self.timeout = timeout;

                Ok(())
            }
            Err(_) => Err(SerialPortError::PortNotAvailable),
        }
    }

    pub async fn disconnect(&mut self) -> Result<(), SerialPortError> {
        if let Some(framed_mutex) = &self.framed {
            let mut framed = framed_mutex.lock().await;
            let _ = framed.flush().await;
            drop(framed);
        }
        self.name = None;
        self.baudrate = 0;
        self.timeout = Duration::from_millis(0);
        self.framed = None;
        self.connected = false;
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
            debug!("Trying to connect to port {:?}", p);
            if let Err(err) = self.connect(&p, baudrate, timeout) {
                debug!("{:?}", err);
                continue;
            }
            if let Err(err) = self.authenticate().await {
                debug!("Disconnecting from port {:?}", p);
                self.disconnect().await.map_err(|_| err)?;
                continue;
            }
            self.connected = true;
            info!("Port {p:?} authentication succeded");
        }
        Err(SerialPortError::PortNotConnected)
    }

    pub async fn authenticate(&mut self) -> Result<(), SerialPortError> {
        self.send_message(&SerialMessage::Ping(PingMessage {}))
            .await?;

        let msg = self.read_message(Duration::from_millis(100)).await?;

        match msg {
            SerialMessage::Pong(_) => {
                debug!("Authentication successful");
                Ok(())
            }
            _ => {
                debug!("Authentication failed");
                Err(SerialPortError::AuthenticationFailed)
            }
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected
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

    pub async fn read_message(
        &self,
        timeout_duration: Duration,
    ) -> Result<SerialMessage, SerialPortError> {
        if let Some(framed_mutex) = &self.framed {
            let mut framed = framed_mutex.lock().await;
            match timeout(timeout_duration, framed.next()).await {
                Ok(Some(Ok(msg))) => Ok(msg),
                Ok(Some(Err(e))) => Err(e),
                Ok(None) => Err(SerialPortError::PortNotConnected),
                Err(_) => Err(SerialPortError::TimedOut),
            }
        } else {
            Err(SerialPortError::PortNotConnected)
        }
    }
}

#[cfg(test)]
mod test {
    use super::Port;
    use crate::messages::ping::PingMessage;
    use crate::messages::pong::PongMessage;
    use crate::serial_message::SerialMessage;

    use tokio::time::Duration;

    #[tokio::test]
    pub async fn test_auto_connect() {
        let mut port = Port::new();
        match port.auto_connect(115200, Duration::from_millis(1000)).await {
            Err(e) => eprintln!("Error: {:?}", e),
            Ok(_) => {}
        }

        assert!(port.is_connected());

        port.disconnect().await.unwrap();
    }

    #[tokio::test]
    pub async fn test_double_ping() {
        let mut port = Port::new();
        match port.auto_connect(115200, Duration::from_millis(1000)).await {
            Err(e) => eprintln!("Error: {:?}", e),
            Ok(_) => {}
        }

        assert!(port.is_connected());

        port.send_message(&SerialMessage::Ping(PingMessage {}))
            .await
            .unwrap();

        let pong = port.read_message(Duration::from_millis(100)).await.unwrap();

        assert_eq!(pong, SerialMessage::Pong(PongMessage {}));

        port.disconnect().await.unwrap();
    }
}
