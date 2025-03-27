use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::discord::client::*;
use crate::discord::error::*;

pub enum DiscordWorkerMessage {
    Stop,
    GetVoiceSettigs,
    SetVoiceSetting(bool, bool),
    Disconnect,
}

pub struct DiscordWorker {
    thread: Option<thread::JoinHandle<()>>,
    tx: Option<mpsc::Sender<DiscordWorkerMessage>>,
    _rx: Option<mpsc::Receiver<DiscordWorkerMessage>>,
    muted: Arc<AtomicBool>,
    deafen: Arc<AtomicBool>,
    config: Arc<Mutex<Option<String>>>,
}

impl DiscordWorker {
    pub fn new() -> DiscordWorker {
        DiscordWorker {
            thread: None,
            tx: None,
            _rx: None,
            muted: Arc::new(AtomicBool::new(false)),
            deafen: Arc::new(AtomicBool::new(false)),
            config: Arc::new(Mutex::new(None)),
        }
    }

    pub fn start(&mut self, ds_token: Option<String>) -> Result<(), DiscordError> {
        let (tx, rx_thread) = mpsc::channel();

        self.tx = Some(tx);

        let muted = self.muted.clone();
        let deafen = self.deafen.clone();
        let conf = self.config.clone();

        let t = thread::spawn(move || {
            let mut ds = DiscordClient::new(
                //FIX this
                "713524519830028368".to_string(),
                ds_token,
                "4Xqsf4ELABGEph3ZsmaaIp3Urr60Ikzp".to_string(),
                "https://www.mechardo3d.xyz/".to_string(),
            );

            loop {
                if !ds.is_connected() {
                    ds.connect_loop();
                    let t = ds.get_token();
                    {
                        *(conf.lock().unwrap()) = t;
                    }
                } else {
                    match ds.get_voice_settings() {
                        Some((m, d)) => {
                            muted.store(m, Ordering::SeqCst);
                            deafen.store(d, Ordering::SeqCst);
                        }
                        None => {}
                    }
                }

                match rx_thread.recv_timeout(Duration::from_millis(10)) {
                    Ok(msg) => match msg {
                        DiscordWorkerMessage::Stop => {
                            break;
                        }
                        DiscordWorkerMessage::GetVoiceSettigs => match ds.get_voice_settings() {
                            Some((m, d)) => {
                                muted.store(m, Ordering::SeqCst);
                                deafen.store(d, Ordering::SeqCst);
                            }
                            None => {}
                        },
                        DiscordWorkerMessage::SetVoiceSetting(m, d) => {
                            ds.set_voice_settings(m, d);
                        }
                        DiscordWorkerMessage::Disconnect => {
                            ds.disconnect();
                        }
                    },
                    Err(_) => { //Ignore
                    }
                }
            }
        });
        self.thread = Some(t);

        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), DiscordError> {
        match &self.tx {
            Some(tx) => {
                tx.send(DiscordWorkerMessage::Stop).unwrap();
            }
            None => {
                return Err(DiscordError::InternalChannelClosed);
            }
        }

        if let Some(handle) = self.thread.take() {
            match handle.join() {
                Ok(_) => {
                    self.thread = None;
                }
                Err(e) => {
                    println!("Error cerrando thread: {:?}", e);
                    return Err(DiscordError::ErrorClosingThread);
                }
            }
        }

        Ok(())
    }

    pub fn get_voice_settings(&mut self) -> Result<(bool, bool), DiscordError> {
        match &self.tx {
            Some(tx) => {
                tx.send(DiscordWorkerMessage::GetVoiceSettigs).unwrap();
            }
            None => {
                return Err(DiscordError::InternalChannelClosed);
            }
        }

        let m = self.muted.load(Ordering::SeqCst);
        let d = self.deafen.load(Ordering::SeqCst); // *self.deafen.get_mut();

        Ok((m, d))
    }

    pub fn set_voice_settings(&mut self, m: bool, d: bool) -> Result<(), DiscordError> {
        match &self.tx {
            Some(tx) => {
                tx.send(DiscordWorkerMessage::SetVoiceSetting(m, d))
                    .unwrap();
            }
            None => {
                return Err(DiscordError::InternalChannelClosed);
            }
        }
        Ok(())
    }

    pub fn disconnect(&mut self) -> Result<(), DiscordError> {
        match &self.tx {
            Some(tx) => {
                tx.send(DiscordWorkerMessage::Disconnect).unwrap();
            }
            None => {
                return Err(DiscordError::InternalChannelClosed);
            }
        }

        Ok(())
    }

    pub fn get_config(&mut self) -> Option<String> {
        let c;
        {
            c = self.config.lock().unwrap().clone();
        }
        c
    }
}
