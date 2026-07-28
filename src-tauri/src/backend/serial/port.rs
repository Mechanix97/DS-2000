use super::error::SerialPortError;
use super::messages::ping::PingMessage;
use super::serial_message::SerialMessage;
use super::serial_message::SerialMessageCodec;

use futures_util::{SinkExt, StreamExt};
use serial2_tokio::SerialPort;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tokio::time::{Duration, timeout};
use tokio_util::codec::Framed;
use tracing::{debug, info};

#[derive(Clone)]
pub struct Port {
    pub name: Option<String>,
    pub baudrate: u32,
    pub timeout: Duration,
    pub framed: Option<Arc<Mutex<Framed<SerialPort, SerialMessageCodec>>>>,
    pub connected: bool,
}

impl Default for Port {
    fn default() -> Self {
        Self::new()
    }
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
        port_name: &Path,
        baudrate: u32,
        timeout: Duration,
    ) -> Result<(), SerialPortError> {
        if self.is_connected() {
            return Err(SerialPortError::PortAlreadyConnected);
        }
        match SerialPort::open(port_name, baudrate) {
            Ok(p) => {
                p.set_dtr(true)?;
                p.set_rts(true)?;

                let framed = Framed::new(p, SerialMessageCodec);
                self.framed = Some(Arc::new(Mutex::new(framed)));
                self.name = port_name.to_str().map(|s| s.to_string());
                self.baudrate = baudrate;
                self.timeout = timeout;

                Ok(())
            }
            Err(err) => {
                debug!("PortNotAvailable: {err}");
                Err(SerialPortError::PortNotAvailable)
            }
        }
    }

    pub async fn disconnect(&mut self) -> Result<(), SerialPortError> {
        if let Some(framed_mutex) = &self.framed {
            let mut framed = framed_mutex.lock().await;
            let port = framed.get_mut();
            port.flush().await?;
            drop(framed);
        }
        if self.connected {
            if let Some(port_name) = &self.name {
                info!("Serial port {port_name} disconnected");
            }
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

            if let Err(err) = self.connect_and_authenticate(&p, baudrate, timeout).await {
                debug!("{:?}", err);
                continue;
            }

            return Ok(());
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

    pub async fn connect_and_authenticate(
        &mut self,
        port_name: &Path,
        baudrate: u32,
        timeout: Duration,
    ) -> Result<(), SerialPortError> {
        self.connect(port_name, baudrate, timeout)?;

        if let Err(err) = self.authenticate().await {
            self.disconnect().await?;
            return Err(err);
        }
        self.connected = true;
        info!("Serial port {port_name:?} connected");
        Ok(())
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
    use tokio::time::sleep;

    #[tokio::test]
    #[ignore = "needs a DS-2000 device connected over USB; run with --ignored"]
    pub async fn test_auto_connect() {
        let mut port = Port::new();
        if let Err(e) = port.auto_connect(115200, Duration::from_millis(1000)).await {
            eprintln!("Error: {:?}", e)
        }

        assert!(port.is_connected());

        port.disconnect().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "needs a DS-2000 device connected over USB; run with --ignored"]
    pub async fn test_double_ping() {
        let mut port = Port::new();
        if let Err(e) = port.auto_connect(115200, Duration::from_millis(1000)).await {
            eprintln!("Error: {:?}", e)
        }

        assert!(port.is_connected());

        port.send_message(&SerialMessage::Ping(PingMessage {}))
            .await
            .unwrap();

        let pong = port.read_message(Duration::from_millis(100)).await.unwrap();

        assert_eq!(pong, SerialMessage::Pong(PongMessage {}));

        port.disconnect().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "needs a DS-2000 device connected over USB; run with --ignored"]
    pub async fn test_disconnect() {
        let mut port = Port::new();
        if let Err(e) = port.auto_connect(115200, Duration::from_millis(1000)).await {
            eprintln!("Error: {:?}", e)
        }

        assert!(port.is_connected());

        port.disconnect().await.unwrap();
        sleep(Duration::from_millis(1000)).await;

        assert!(!port.is_connected());

        if let Err(e) = port.auto_connect(115200, Duration::from_millis(1000)).await {
            eprintln!("Error: {:?}", e)
        }

        assert!(port.is_connected());

        port.disconnect().await.unwrap();
        sleep(Duration::from_millis(1000)).await;

        assert!(!port.is_connected());
    }
}
