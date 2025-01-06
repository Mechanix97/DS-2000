pub mod discord;

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

    match ds.get_voice_settings() {
        Ok(r) => {   
            println!("get_voice_settings: {}, {}", r.0,  r.1);
        }
        Err(e) => {
            println!("error re loco3. {:?}", e);
        }
    }
}
