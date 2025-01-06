use std::io::{Write, Read};
use named_pipe::PipeClient;


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
   

    pub fn handshake(&mut self, client_id: String) -> Result<(), DiscordError> {
        let hm =  PipeMessage::handshake(&client_id);
        
        self.client_id = Some(client_id);

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


    pub fn read_message(&mut self) -> Result<PipeMessage, DiscordError> {
        let mut buf = [0u8; 4];
        let received_opcode: u32;
        let received_length: u32;
        
        match &mut self.client_pipe {
            Some(cp) => {
                if let Err(_) = cp.read_exact(&mut buf){
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

    pub fn authorize(&mut self) -> Result<String, DiscordError> {
        let am;
        match &self.client_id{
            Some(c) => {
                am = PipeMessage::authorize(&c, "rpc");
            }
            None => {
                return Err(DiscordError::ClientIdNotFound)
            }
        }
        println!("{:?}", am);
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
        
        match self.read_message() {
            Ok(m) => {
                let parsed_json: serde_json::Value = serde_json::from_str(&m.payload.unwrap()).expect("Error al analizar JSON");
                println!("parsed json: {:?}", parsed_json);
                Ok(parsed_json["data"]["code"].to_string())
            }
            Err(e) => {
                return Err(e.clone())
            }
        }
    }


    // pub fn authenticate(&mut self, token: &str) -> Result<(),()>{
    //     let am = PipeMessage::authenticate(token);
    //     self.client.write_all(&am.to_buff()).unwrap();
        
    //     match self.read_message() {
    //         Ok(m) => {
    //             println!("{}",&m.payload.clone().unwrap());
    //             let parsed_json: serde_json::Value = serde_json::from_str(&m.payload.clone().unwrap()).expect("Error al analizar JSON");
    //             if parsed_json["data"]["evt"] != "ERROR" && parsed_json["data"]["code"] != 4009 {
    //                 println!("AUTHENTICATED!");   
    //                 Ok(())
    //             } else {
    //                 Err(())   

    //             }
    //         }
    //         Err(e) => {
    //             println!("{}", e);
    //             Err(())
    //         }
    //     }
    // }
}