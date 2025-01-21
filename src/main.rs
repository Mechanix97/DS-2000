pub mod discord;
pub mod config;
pub mod serial;

use std::{thread, time};

// use core::time;
use std::time::Duration;

use discord::client::DiscordClient;
use serial::port::Port;
use serial::worker::SerialWorker;
// use config::config::Config;


fn main(){
    // let mut ds = DiscordClient::new(
    //     "713524519830028368".to_string(),
    //     Some("S8ngQYkWFytsdOsr0W1ULVlo9XQk2y".to_string()),
    //     "4Xqsf4ELABGEph3ZsmaaIp3Urr60Ikzp".to_string(),
    //     "https://www.mechardo3d.xyz/".to_string()
    // );

    // ds.connect_loop();

    // let mut port = Port::new();

    // let ports = port.get_ports().unwrap();

    // for p in ports {
    //     println!("{}", p);
    // }

    // port.auto_connect(9600, Duration::from_millis(100));

    let mut sw = SerialWorker::new();
    sw.start(None);
    
thread::sleep(time::Duration::from_secs(15));

    sw.stop();
    

}


// fn main() {
//     let mut config = Config::new();
//     config.load();



//     let mut ds = IPCClient::new();
//     match ds.connect() {
//         Ok(_) => {
//             println!("CONECTADO CON DS");
//         }
//         Err(_) => {
//             println!("error re loco");
//         }
//     } 

//     match ds.handshake("713524519830028368".to_string()) {
//         Ok(_) => {
//             println!("HANDSHAKE OK");
//         }
//         Err(_) => {
//             println!("error re loco");
//         }
//     } 

//     let mut code = "".to_string();
//     match ds.authorize() {
//         Ok(t) => {
//             code = t.clone();
//             println!("AUTHORIZE OK. code: {}", t);
//         }
//         Err(_) => {
//             println!("error re loco");
//         }
//     }

//     code = code.trim_matches('"').to_owned();
//     let token = ds.get_access_token(&code, "4Xqsf4ELABGEph3ZsmaaIp3Urr60Ikzp", "https://www.mechardo3d.xyz/");
//     println!("TOKEN: {}", token);
//     match ds.authenticate(&token) {
//         Ok(_) => {   
//             println!("authenticate OK.");
//         }
//         Err(e) => {
//             println!("error re loco2. {:?}", e);
//         }
//     }

//     loop{
//         let ten_millis = time::Duration::from_millis(10000);
        
//     thread::sleep(ten_millis);
//          ds.select_voice_channel(None).unwrap();
//         // match ds.get_voice_settings() {
//         //     Ok(r) => {   
//         //         println!("get_voice_settings: {}, {}", r.0,  r.1);
//         //         ds.set_voice_settings(!r.0, r.1).unwrap();
//         //         ds.set_voice_settings(!r.0, !r.1).unwrap();
//         //     }
//         //     Err(e) => {
//         //         println!("error re loco3. {:?}", e);
//         //     }
//         // }
//     }
// }
