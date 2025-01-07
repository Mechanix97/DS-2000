use std::io::{Write, Read};
use named_pipe::PipeClient;
use serde_json::Value;
use std::collections::HashMap;

use crate::discord::error::*;
use crate::discord::pipemessage::*;

pub struct IPCClient {
    client_pipe: Option<PipeClient>,
    client_id: Option<String>
}

impl IPCClient {
    pub fn new() -> Self {
        Self{
            client_pipe: None,
            client_id: None
        }
    }

    pub fn connect(&mut self) -> Result<(), DiscordError> {
        let iter = 0..10;
        let last: i32 = iter.end - 1;
        for i in iter {
            let pipe_name = format!(r"\\?\pipe\discord-ipc-{}", i);
            match PipeClient::connect(pipe_name) {
                Ok(client) => {
                    self.client_pipe = Some(client);
                    return Ok(())
                }
                Err(_) => { 
                    if i == last {
                        return Err(DiscordError::PipeConnectionFailed)
                    } else { 
                        continue 
                    }       
                }
            }
        }

        return Err(DiscordError::PipeConnectionFailed)
    }
   

    pub fn read_message(&mut self) -> Result<PipeMessage, DiscordError> {
        let mut buf = [0u8; 4];
        let received_opcode: u32;
        let received_length: u32;
        
        match &mut self.client_pipe {
            Some(cp) => {
                if let Err(e) = cp.read_exact(&mut buf){
                    println!("ERROR EN READ: {:?}", e);
                    return Err(DiscordError::PipeErrorReading);
                }
                received_opcode = u32::from_le_bytes(buf);

                if let Err(_) = cp.read_exact(&mut buf){
                    return Err(DiscordError::PipeErrorReading);
                }
                received_length = u32::from_le_bytes(buf);

                let mut response_data = vec![0u8; received_length as usize];
                if let Err(_) = cp.read_exact(&mut response_data){
                    return Err(DiscordError::PipeErrorReading);
                }
                let response_data_str = String::from_utf8_lossy(&response_data);
                 
                return Ok(PipeMessage::new(Opcode::new(received_opcode), &response_data_str))
            },
            None => {
                return Err(DiscordError::PipeNotConnected)
            }
        }
    }

    pub fn handshake(&mut self, client_id: String) -> Result<(), DiscordError> {
        //build message
        let hm =  PipeMessage::handshake(&client_id);
        
        //store client id
        self.client_id = Some(client_id);

        //send message
        match &mut self.client_pipe{
            Some(cp) => {
                match cp.write_all(&hm.to_buff()) {
                    Ok(_) => {}
                    Err(_) => {
                        return Err(DiscordError::PipeWriteError)
                    }
                }
            }
            None => {
                return Err(DiscordError::PipeNotConnected)
            }
        }
        
        //receive reply
        match &self.read_message() {
            Ok(m) => {
                if m.opcode == Opcode::Frame {
                    return Ok(());
                } else {
                    return Err(DiscordError::HandshakeFailed);
                }
            }
            Err(e) => {
                return Err(e.clone())
            }
        }
    }


    
    pub fn authorize(&mut self) -> Result<String, DiscordError> {
        //build message
        let am;
        match &self.client_id{
            Some(c) => {
                am = PipeMessage::authorize(&c, "rpc");
            }
            None => {
                return Err(DiscordError::ClientIdNotFound)
            }
        }
        
        //send message
        match &mut self.client_pipe {
            Some(cp) => {
                match cp.write_all(&am.to_buff()) {
                    Ok(_) => {}
                    Err(_) => {
                        return Err(DiscordError::PipeWriteError)
                    }
                }
            }
            None => {
                return Err(DiscordError::PipeNotConnected)
            }
        }
        
        //receive reply
        match self.read_message() {
            Ok(m) => {
                let parsed_json: serde_json::Value = match serde_json::from_str(&m.payload.unwrap()) {
                    Ok(payload) =>{ payload}
                    Err(_) => {return Err(DiscordError::SerdeConvertionError);}
                };
                if !( parsed_json["evt"].is_null()){
                    return Err(DiscordError::AuthorizationFailed);
                }
                Ok(parsed_json["data"]["code"].to_string())
            }
            Err(e) => {
                return Err(e.clone())
            }
        }
    }


    pub fn authenticate(&mut self, token: &str) -> Result<(), DiscordError>{
        //build message
        let am = PipeMessage::authenticate(token);
        
        //send message
        match &mut self.client_pipe {
            Some(cp) => {
                match cp.write_all(&am.to_buff()) {
                    Ok(_) => {}
                    Err(_) => {
                        return Err(DiscordError::PipeWriteError)
                    }
                }
            }
            None => {
                return Err(DiscordError::PipeNotConnected)
            }
        }
        
        //receive reply
        match self.read_message() {
            Ok(m) => {
                let parsed_json: serde_json::Value = match serde_json::from_str(&m.payload.unwrap()) {
                    Ok(payload) =>{ payload}
                    Err(_) => {return Err(DiscordError::SerdeConvertionError);}
                };
                if !( parsed_json["evt"].is_null()){
                    return Err(DiscordError::AuthenticationFailed);
                }
                Ok(())
            }
            Err(e) => {
                return Err(e.clone())
            }
        }
    }

    pub fn get_access_token(&mut self, code: &str, client_secret: &str, redirect_uri: &str) -> String {
        let api_endpoint = "https://discord.com/api/v10/oauth2/token";
        let cs = client_secret.to_string();
        let ci = self.client_id.clone().unwrap();
        let ac = "authorization_code".to_string();
        let c = code.to_string();
        let ru = redirect_uri.to_string();
        let mut data = HashMap::new();
  
        println!("code c: {}", c);
        data.insert("client_id", &ci);
        data.insert("client_secret", &cs);
        data.insert("grant_type", &ac);
        data.insert("code", &c);
        data.insert("redirect_uri", &ru);

        let ds = reqwest::blocking::Client::new();
        let res = ds.post(api_endpoint)
            .form(&data)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .send().unwrap();
        let body = res.text().unwrap();
        let response: Value = serde_json::from_str(&body).unwrap();
        println!("Discord api response: {}", response);
        
        response["access_token"].to_string().trim_matches('"').to_string()
    }  


    pub fn get_voice_settings(&mut self) -> Result<(bool, bool), DiscordError>{
        //build messsage
        let gvsm: PipeMessage = PipeMessage::get_voice_settings();

        //send message
        match &mut self.client_pipe {
            Some(cp) => {
                match cp.write_all(&gvsm.to_buff()) {
                    Ok(_) => {}
                    Err(e) => {
                        println!("{:?}", e);
                        return Err(DiscordError::PipeWriteError)
                    }
                }
            }
            None => {
                return Err(DiscordError::PipeNotConnected)
            }
        }
        
        //receive reply
        match self.read_message() {
            Ok(m) => {
                let parsed_json: serde_json::Value = match serde_json::from_str(&m.payload.unwrap()) {
                    Ok(payload) =>{ payload}
                    Err(_) => {return Err(DiscordError::SerdeConvertionError);}
                };
                if !( parsed_json["evt"].is_null()){
                    return Err(DiscordError::AuthenticationFailed);
                }
                if parsed_json["data"]["mute"].is_null() || parsed_json["data"]["deaf"].is_null() {
                    return Err(DiscordError::NoDataFound)
                }
                let muted = parsed_json["data"]["mute"].as_bool().unwrap();
                let deafen =  parsed_json["data"]["deaf"].as_bool().unwrap();
                Ok((muted, deafen))
            }
            Err(e) => {
                return Err(e.clone())
            }
        }      
    }

    pub fn set_voice_settings(&mut self, muted: bool, deafed: bool) -> Result<(), DiscordError> {
        //build message
        let svsm: PipeMessage = PipeMessage::set_voice_settings(muted, deafed);
        
        //send message
        match &mut self.client_pipe {
            Some(cp) => {
                match cp.write_all(&svsm.to_buff()) {
                    Ok(_) => {}
                    Err(e) => {
                        println!("{:?}", e);
                        return Err(DiscordError::PipeWriteError)
                    }
                }
            }
            None => {
                return Err(DiscordError::PipeNotConnected)
            }
        }
                
       //receive reply
        match self.read_message() {
            Ok(m) => {
                let parsed_json: serde_json::Value = match serde_json::from_str(&m.payload.unwrap()) {
                    Ok(payload) =>{ payload}
                    Err(_) => {return Err(DiscordError::SerdeConvertionError);}
                };
                if !( parsed_json["evt"].is_null()){
                    return Err(DiscordError::AuthenticationFailed);
                }
                Ok(())
            }
            Err(e) => {
                return Err(e.clone())
            }
        }
    }
    

    pub fn select_voice_channel(&mut self, channel_id: Option<String>) -> Result<(), DiscordError>{
        //build message
        let svc: PipeMessage = PipeMessage::select_voice_channel(channel_id);

        //send message
        match &mut self.client_pipe {
            Some(cp) => {
                match cp.write_all(&svc.to_buff()) {
                    Ok(_) => {}
                    Err(e) => {
                        println!("{:?}", e);
                        return Err(DiscordError::PipeWriteError);
                    }
                }
            }
            None => {
                return Err(DiscordError::PipeNotConnected)
            }
        };
        
        //receive reply
        match self.read_message() {
            Ok(m) => {
                let parsed_json: serde_json::Value = match serde_json::from_str(&m.payload.unwrap()) {
                    Ok(payload) =>{payload}
                    Err(_) => {return Err(DiscordError::SerdeConvertionError);}
                };
                if !( parsed_json["evt"].is_null()){
                    return Err(DiscordError::AuthenticationFailed);
                }
                Ok(())
            }
            Err(e) => {
                return Err(e.clone())
            }
        }
    }

}