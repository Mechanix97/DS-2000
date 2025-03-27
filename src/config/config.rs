use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub discord_client_id: Option<String>,
    pub discord_secret_key: Option<String>,
    pub discord_access_token: Option<String>,
    pub discord_refresh_token: Option<String>,
    pub last_port_connected: Option<String>,
}

impl Config {
    pub fn new() -> Self {
        Self {
            discord_client_id: Some("713524519830028368".to_string()),
            discord_secret_key: Some("FfYWvhnxrLlfFqovVfZUl7_CPz6W5Zz5".to_string()),
            discord_access_token: None,
            discord_refresh_token: None,
            last_port_connected: None,
        }
    }

    pub fn load(&mut self) {
        if let Some(base_dirs) = BaseDirs::new() {
            let appdata_path = base_dirs.config_dir().join("Mechardo");
            if !appdata_path.exists() {
                std::fs::create_dir_all(&appdata_path).unwrap();
                return;
            }
            let config_file = appdata_path.join("ds-config.json");
            if !Path::new(&config_file).exists() {
                File::create(appdata_path.join("ds-config.json")).unwrap();
            }
            match File::open(config_file) {
                Ok(f) => {
                    *self = serde_json::from_reader(f).unwrap();
                }
                Err(e) => {
                    println!("Error: {}", e);
                }
            };
        }
    }

    pub fn save(&mut self) {
        if let Some(base_dirs) = BaseDirs::new() {
            let appdata_path = base_dirs.config_dir().join("Mechardo");
            if !appdata_path.exists() {
                std::fs::create_dir_all(&appdata_path).unwrap();
            }
            let file = File::create(appdata_path.join("ds-config.json")).unwrap();
            serde_json::to_writer_pretty(file, self).unwrap();
        }
    }

    // pub fn set_discord_access_token(&mut self, token: String) {
    //     self.discord_access_token = Some(token);
    // }

    // pub fn get_discord_access_token(self) -> Option<String> {
    //     self.discord_access_token
    // }

    // pub fn set_last_port_connected(&mut self, port: String) {
    //     self.last_port_connected = Some(port);
    // }

    // pub fn get_last_port_connected(&self) -> Option<String> {
    //     self.last_port_connected
    // }
}
