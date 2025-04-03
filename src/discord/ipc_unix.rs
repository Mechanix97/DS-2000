// use serde_json::Value;
// use std::collections::HashMap;
// use std::io::{Read, Write};
use std::{env::var, os::unix::net::UnixStream};


use crate::discord::error::*;
// use crate::discord::pipemessage::*;

pub struct IPCClient {
    client_pipe: Option<UnixStream>,
    client_id: Option<String>,
}

impl IPCClient {
    pub fn new() -> Self {
        Self {
            client_pipe: None,
            client_id: None,
        }
    }

    pub fn connect(&mut self) -> Result<(), DiscordError> {
        let mut sub_path = None;    
        for key in ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]{
            if let Ok(env_var) = var(key){
                sub_path = Some(env_var);
            }
        }
        match sub_path {
            None => { return Err(DiscordError::PipeConnectionFailed); }
            Some(sp) => {
                for i in 0..10 {
                    let pipe_name = format!("{}discord-ipc-{}", sp, i);
                    match UnixStream::connect(&pipe_name) {
                        Err(_) => {continue;}
                        Ok(pipe) =>{
                            self.client_pipe = Some(pipe);
                            return Ok(());
                        }
                    }
                }
            }
        }
        Err(DiscordError::PipeConnectionFailed)
    }

    pub fn read_message(&mut self) -> Result<(), DiscordError> {
        Ok(())
    }

    pub fn handshake(&mut self, client_id: String) -> Result<(), DiscordError> {
        Ok(())
    }

    pub fn authorize(&mut self) -> Result<String, DiscordError> {
        Ok("HOLA".to_string())
    }

    pub fn authenticate(&mut self, token: &str) -> Result<(), DiscordError> {
        Ok(())
    }

    pub fn get_access_token(
        &mut self,
        code: &str,
        client_secret: &str,
        redirect_uri: &str,
    ) -> String {
        "HOLA".to_string()
    }

    pub fn get_voice_settings(&mut self) -> Result<(bool, bool), DiscordError> {
        Ok((false,false))
    }

    pub fn set_voice_settings(&mut self, muted: bool, deafed: bool) -> Result<(), DiscordError> {
       Ok(())
    }

    pub fn select_voice_channel(&mut self, channel_id: Option<String>) -> Result<(), DiscordError> {
       Ok(())
    }
}
