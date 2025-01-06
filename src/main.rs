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

   match ds.handshake("1325566540879237140".to_string()) {
    Ok(_) => {
     println!("HANDSHAKE OK");
    }
    Err(_) => {
     println!("error re loco");
    }
} 
}
