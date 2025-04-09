use crate::backend::discord::error::*;
use crate::backend::discord::ipc::*;

#[derive(Debug)]
pub enum DiscordStatus {
    NotConnected,
    Connected,
    HandshakeOk,
    Authenticated,
}

pub struct DiscordClient {
    ipc_client: IPCClient,
    status: DiscordStatus,
    client_id: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
    client_secret: String,
    redirect_uri: String,
}

impl DiscordClient {
    pub fn new(
        client_id: String,
        access_token: Option<String>,
        refresh_token: Option<String>,
        client_secret: String,
        redirect_uri: String,
    ) -> Self {
        Self {
            ipc_client: IPCClient::new(),
            status: DiscordStatus::NotConnected,
            client_id: client_id,
            access_token: access_token,
            refresh_token: refresh_token,
            client_secret: client_secret,
            redirect_uri: redirect_uri,
        }
    }

    pub fn connect(&mut self) {
        match self.status {
            DiscordStatus::NotConnected => match self.ipc_client.connect() {
                Ok(_) => {
                    self.status = DiscordStatus::Connected;
                }
                Err(DiscordError::PipeConnectionFailed) => {
                    self.status = DiscordStatus::NotConnected;
                }
                Err(e) => {
                    panic!("No deberia estar aca: {:?}", e);
                }
            },
            _ => {}
        }
    }

    pub fn handshake(&mut self) {
        match self.status {
            DiscordStatus::Connected => match self.ipc_client.handshake(self.client_id.clone()) {
                Ok(_) => {
                    self.status = DiscordStatus::HandshakeOk;
                }
                Err(DiscordError::PipeNotConnected) => {
                    self.status = DiscordStatus::NotConnected;
                }
                Err(e) => {
                    panic!("No deberia estar aca: {:?}", e);
                }
            },
            _ => {} //Por ahora no hago nada
        }
    }

    pub fn authorize(&mut self) -> Option<String> {
        match self.status {
            DiscordStatus::HandshakeOk => match self.ipc_client.authorize() {
                Ok(code) => Some(code.trim_matches('"').to_owned()),
                Err(DiscordError::PipeNotConnected) => {
                    self.status = DiscordStatus::NotConnected;
                    None
                }
                Err(e) => {
                    panic!("No deberia estar aca: {:?}", e);
                }
            },
            _ => None,
        }
    }

    pub fn authenticate(&mut self) {
        match self.status {
            DiscordStatus::HandshakeOk => match &self.access_token {
                Some(token) => match self.ipc_client.authenticate(&token) {
                    Ok(_) => {
                        self.status = DiscordStatus::Authenticated;
                    }
                    Err(DiscordError::PipeNotConnected) => {
                        self.status = DiscordStatus::NotConnected;
                    }
                    Err(DiscordError::AuthenticationFailed) => {
                        self.access_token = None;
                        self.refresh_token = None;
                    }
                    Err(e) => {
                        panic!("No deberia estar aca: {:?}", e);
                    }
                },
                None => match self.authorize() {
                    Some(code) => {
                        let (access_token, refresh_token) = self.ipc_client.get_tokens(
                            &code,
                            &self.client_secret,
                            &self.redirect_uri,
                        );
                        self.access_token = Some(access_token);
                        self.refresh_token = Some(refresh_token);
                        self.authenticate();
                    }
                    None => {}
                },
            },
            _ => {}
        }
    }

    pub fn connect_loop(&mut self) {
        match self.status {
            DiscordStatus::NotConnected => {
                self.connect();
            }
            DiscordStatus::Connected => {
                self.handshake();
            }
            DiscordStatus::HandshakeOk => {
                self.authenticate();
            }
            DiscordStatus::Authenticated => {}
        }
    }

    pub fn is_connected(&mut self) -> bool {
        match self.status {
            DiscordStatus::Authenticated => true,
            _ => false,
        }
    }

    pub fn get_voice_settings(&mut self) -> Option<(bool, bool)> {
        match self.status {
            DiscordStatus::Authenticated => match self.ipc_client.get_voice_settings() {
                Ok(vs) => Some(vs),
                Err(_) => {
                    self.status = DiscordStatus::NotConnected;
                    None
                }
            },
            _ => None,
        }
    }

    pub fn set_voice_settings(&mut self, m: bool, d: bool) {
        match self.status {
            DiscordStatus::Authenticated => {
                self.ipc_client.set_voice_settings(m, d).unwrap();
            }

            _ => {}
        }
    }

    pub fn disconnect(&mut self) {
        match self.status {
            DiscordStatus::Authenticated => {
                self.ipc_client.select_voice_channel(None).unwrap();
            }

            _ => {}
        }
    }

    pub fn get_access_token(&mut self) -> String {
        match &self.access_token {
            Some(at) => at.clone(),
            None => "".to_string(),
        }
    }

    pub fn get_refresh_token(&mut self) -> String {
        match &self.refresh_token {
            Some(at) => at.clone(),
            None => "".to_string(),
        }
    }
}
