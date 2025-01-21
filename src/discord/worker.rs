use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::discord::error::*;
use crate::discord::client::*;

pub enum DiscordWorkerMessage{
    Stop,
    GetVoiceSettigs
}


pub struct DiscordWorker {
    thread: Option<thread::JoinHandle<()>>,
    tx: Option<mpsc::Sender<DiscordWorkerMessage>>,
    rx: Option<mpsc::Receiver<DiscordWorkerMessage>>,
    muted: Arc<AtomicBool>,
    deafen: Arc<AtomicBool>
}


impl DiscordWorker{
    pub fn new() -> DiscordWorker {
        DiscordWorker{
            thread: None,
            tx: None,
            rx: None,
            muted: Arc::new(AtomicBool::new(false)),
            deafen: Arc::new(AtomicBool::new(false)),
        }
    }


    pub fn start(&mut self) -> Result<(), DiscordError> {
        let (tx, rx_thread) = mpsc::channel();
        let (tx_thread, rx) = mpsc::channel(); 
        self.tx = Some(tx);
        self.rx = Some(rx);
        // let mut st = self.status.clone();

        let mut muted = self.muted.clone();
        let mut deafen = self.deafen.clone();

        let t = thread::spawn(move || {
            let mut ds =  DiscordClient::new( //FIX this
                    "713524519830028368".to_string(),
                    Some("S8ngQYkWFytsdOsr0W1ULVlo9XQk2y".to_string()),
                    "4Xqsf4ELABGEph3ZsmaaIp3Urr60Ikzp".to_string(),
                    "https://www.mechardo3d.xyz/".to_string()
                );

                

                loop{
                    if !ds.is_connected(){
                        ds.connect_loop();
                    } else{
                        match ds.get_voice_settings() {
                            Some((m,d)) => {
                                muted.store(m, Ordering::SeqCst);
                                deafen.store(d, Ordering::SeqCst);
                            }
                            None => {}
                        }
                    }
                    
                    // println!("|En el proceso|Muted: {} | Deafen {}", muted, deafen);
                    // println!("Deafen: {}", deafen);
                    
                    match rx_thread.recv_timeout(Duration::from_millis(10)){
                        Ok(msg) => {
                            match msg {
                                DiscordWorkerMessage::Stop => {
                                    break;
                                }
                                DiscordWorkerMessage::GetVoiceSettigs => {
                                    match ds.get_voice_settings() {
                                        Some((m,d)) => {
                                            muted.store(m, Ordering::SeqCst);
                                            deafen.store(d, Ordering::SeqCst);         
                                        }
                                        None => {}
                                    }
                                }
                                _ => {}
                            }
                        }
                        Err(e) => {
                            // println!("Error: {}", e);
                        }
                    } 
                }
        });
        self.thread = Some(t);

        Ok(())
    }


    
    pub fn stop(&mut self) -> Result<(), DiscordError>{
        match &self.tx {
            Some(tx) => {
                tx.send(DiscordWorkerMessage::Stop).unwrap();
            }
            None => {
                return Err(DiscordError::InternalChannelClosed);
            }
        }

        if let Some(handle) = self.thread.take() {
            match handle.join(){
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


    pub fn get_voice_settings(&mut self) ->  Result<(bool, bool), DiscordError>{
        match &self.tx {
            Some(tx) => {
                tx.send(DiscordWorkerMessage::GetVoiceSettigs).unwrap();
            }
            None => {
                return Err(DiscordError::InternalChannelClosed);
            }
        }


        let m = self.muted.load(Ordering::SeqCst);
        let d =self.deafen.load(Ordering::SeqCst);// *self.deafen.get_mut();

        Ok((m,d))
        // match &self.rx {
        //     Some(rx) => {
        //         match rx.recv_timeout(Duration::from_millis(100)){
        //             Ok(msg) => {
        //                 match msg {
        //                     DiscordWorkerMessage::GetVoiceSettigsReply(m,d ) => {
        //                         return Ok((m,d));
        //                     }
        //                     _ => {
        //                         return Err(DiscordError::InvalidChanelMessage);
        //                     }
        //                 }
        //             }
        //             Err(e) => {
        //                 return Err(DiscordError::InternalChannelClosed);
        //             }
        //         }
        //     }
        //     None => {
        //         return Err(DiscordError::InternalChannelClosed);
        //     }            
        // }
    }
}