use std::sync::mpsc;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

use crate::backend::discord::client::*;
use crate::backend::discord::error::*;
use crate::config::DSConfig;

const REDIRECT_URI: &str = "https://www.mechardo3d.xyz/";

enum DiscordWorkerMessage {
    Stop,
    GetVoiceSettigs,
    SetVoiceSetting(bool, bool),
    Disconnect,
}

pub enum DiscordUpdate {
    NewAccessToken(String),
    NewRefreshToken(String),
    NewDiscordVoiceSetting(bool, bool),
}

struct DiscordState {
    muted: bool,
    deafen: bool,
    updates: Vec<DiscordUpdate>,
    acces_token: Option<String>,
    refresh_token: Option<String>,
}

impl DiscordState {
    pub fn new() -> Self {
        DiscordState {
            muted: false,
            deafen: false,
            updates: vec![],
            acces_token: None,
            refresh_token: None,
        }
    }

    pub fn write_state(&mut self, muted: bool, deafen: bool) {
        self.muted = muted;
        self.deafen = deafen;
    }

    pub fn update_state(&mut self, muted: bool, deafen: bool) {
        if self.muted != muted || self.deafen != deafen {
            self.updates
                .push(DiscordUpdate::NewDiscordVoiceSetting(muted, deafen));
        }
        self.muted = muted;
        self.deafen = deafen;
    }

    pub fn get_state(&self) -> (bool, bool) {
        (self.muted, self.deafen)
    }

    pub fn has_update(&self) -> bool {
        self.updates.len() > 0
    }

    pub fn get_update(&mut self) -> Option<DiscordUpdate> {
        self.updates.pop()
    }

    pub fn save_tokens(&mut self, access_token: String, refresh_token: String) {
        self.acces_token = Some(access_token.clone());
        self.refresh_token = Some(refresh_token.clone());
        self.updates
            .push(DiscordUpdate::NewAccessToken(access_token));
        self.updates
            .push(DiscordUpdate::NewRefreshToken(refresh_token));
    }
}

pub struct DiscordWorker {
    thread: Option<thread::JoinHandle<()>>,
    tx: Option<mpsc::Sender<DiscordWorkerMessage>>,
    _rx: Option<mpsc::Receiver<DiscordWorkerMessage>>,
    state: Arc<RwLock<DiscordState>>,
}

impl DiscordWorker {
    pub fn new() -> DiscordWorker {
        DiscordWorker {
            thread: None,
            tx: None,
            _rx: None,
            state: Arc::new(RwLock::new(DiscordState::new())),
        }
    }

    pub fn start(&mut self, config: DSConfig) -> Result<(), DiscordError> {
        let (tx, rx_thread) = mpsc::channel();

        self.tx = Some(tx);

        let state = self.state.clone();

        let t = thread::spawn(move || {
            let mut ds = DiscordClient::new(
                config.discord_client_id.clone().unwrap(),
                config.discord_access_token.clone(),
                config.discord_refresh_token.clone(),
                config.discord_secret_key.clone().unwrap(),
                REDIRECT_URI.to_string(),
            );

            loop {
                if !ds.is_connected() {
                    ds.connect_loop();
                    if ds.is_connected() {
                        state
                            .write()
                            .unwrap()
                            .save_tokens(ds.get_access_token(), ds.get_refresh_token());
                    }
                }

                match rx_thread.recv_timeout(Duration::from_millis(10)) {
                    Ok(msg) => match msg {
                        DiscordWorkerMessage::Stop => {
                            break;
                        }
                        DiscordWorkerMessage::GetVoiceSettigs => match ds.get_voice_settings() {
                            Some((m, d)) => {
                                state.write().unwrap().update_state(m, d);
                            }
                            None => {}
                        },
                        DiscordWorkerMessage::SetVoiceSetting(m, d) => {
                            state.write().unwrap().write_state(m, d);
                            ds.set_voice_settings(m || d, d);
                        }
                        DiscordWorkerMessage::Disconnect => {
                            ds.disconnect();
                        }
                    },
                    Err(_) => { //Ignore
                    }
                }
                match ds.get_voice_settings() {
                    Some((m, d)) => {
                        state.write().unwrap().update_state(m, d);
                    }
                    None => {}
                };
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

        let (m, d) = self.state.read().unwrap().get_state();
        Ok((m, d))
    }

    pub fn set_voice_settings(&mut self, m: bool, d: bool) -> Result<(), DiscordError> {
        match &self.tx {
            Some(tx) => tx
                .send(DiscordWorkerMessage::SetVoiceSetting(m, d))
                .map_err(|_| DiscordError::InternalChannelClosed)?,
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

    pub fn has_update(&self) -> bool {
        self.state.read().unwrap().has_update()
    }

    pub fn get_update(&self) -> Option<DiscordUpdate> {
        self.state.write().unwrap().get_update()
    }
}
