pub mod discord;
use std::{thread, time};

use discord::ipc::IPCClient;

fn main() {
    let mut ds = IPCClient::new();
    match ds.connect() {
        Ok(_) => {
            println!("CONECTADO CON DS");
        }
        Err(_) => {
            println!("error re loco");
        }
    } 

    match ds.handshake("713524519830028368".to_string()) {
        Ok(_) => {
            println!("HANDSHAKE OK");
        }
        Err(_) => {
            println!("error re loco");
        }
    } 

    let mut code = "".to_string();
    match ds.authorize() {
        Ok(t) => {
            code = t.clone();
            println!("AUTHORIZE OK. code: {}", t);
        }
        Err(_) => {
            println!("error re loco");
        }
    }

    code = code.trim_matches('"').to_owned();
    let token = ds.get_access_token(&code, "4Xqsf4ELABGEph3ZsmaaIp3Urr60Ikzp", "https://www.mechardo3d.xyz/");
    println!("TOKEN: {}", token);
    match ds.authenticate(&token) {
        Ok(_) => {   
            println!("authenticate OK.");
        }
        Err(e) => {
            println!("error re loco2. {:?}", e);
        }
    }

    loop{
        let ten_millis = time::Duration::from_millis(10000);
        
    thread::sleep(ten_millis);
         ds.select_voice_channel(None).unwrap();
        // match ds.get_voice_settings() {
        //     Ok(r) => {   
        //         println!("get_voice_settings: {}, {}", r.0,  r.1);
        //         ds.set_voice_settings(!r.0, r.1).unwrap();
        //         ds.set_voice_settings(!r.0, !r.1).unwrap();
        //     }
        //     Err(e) => {
        //         println!("error re loco3. {:?}", e);
        //     }
        // }
    }
}
