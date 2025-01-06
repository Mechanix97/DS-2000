use std::io::{Write, Read};

use named_pipe::PipeClient;
use crate::discord::error::*;
use crate::discord::pipemessage::*;

pub struct IPCClient {
    client_pipe: Option<PipeClient>
}

impl IPCClient {
    pub fn new() -> Self {
        Self{
            client_pipe: None
        }
    }

    pub fn connect(&mut self) -> Result<(), DiscordErrors> {
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
                        return Err(DiscordErrors::PipeConnectionFailed)
                    } else { 
                        continue 
                    }       
                }
            }
        }

        return Err(DiscordErrors::PipeConnectionFailed)
    }
   

    pub fn handshake(&mut self, client_id: String) -> Result<(), DiscordErrors> {
        let hm =  PipeMessage::handshake(&client_id);
        match &mut self.client_pipe{
            Some(cp) => {
                cp.write_all(&hm.to_buff()).unwrap();
            }
            None => {
                return Err(DiscordErrors::PipeNotConnected)
            }
        }
        
        match &self.read_message() {
            Ok(m) => {
                if m.opcode == Opcode::Frame {
                    return Ok(());
                } else {
                    return Err(DiscordErrors::HandshakeFailed);
                }
            }
            Err(e) => {
                return Err(e.clone())
            }
        }
    }


    pub fn read_message(&mut self) -> Result<PipeMessage, DiscordErrors> {
        let mut buf = [0u8; 4];
        let received_opcode: u32;
        let received_length: u32;
        
        match &mut self.client_pipe {
            Some(cp) => {
                if let Err(_) = cp.read_exact(&mut buf){
                    return Err(DiscordErrors::PipeErrorReading);
                }
                received_opcode = u32::from_le_bytes(buf);
                if let Err(_) = cp.read_exact(&mut buf){
                    return Err(DiscordErrors::PipeErrorReading);
                }
                received_length = u32::from_le_bytes(buf);
                let mut response_data = vec![0u8; received_length as usize];
                if let Err(_) = cp.read_exact(&mut response_data){
                    return Err(DiscordErrors::PipeErrorReading);
                }
                let response_data_str = String::from_utf8_lossy(&response_data);
                 
                return Ok(PipeMessage::new(Opcode::new(received_opcode), &response_data_str))
            },
            None => {
                return Err(DiscordErrors::PipeNotConnected)
            }
        }
    }
}