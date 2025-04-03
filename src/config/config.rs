use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use directories::BaseDirs;
use dotenvy;
use hex::decode;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::env::var;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

const ENV_FILEPATH: &str = "secrets/discord.env";

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
        dotenvy::from_path(Path::new(ENV_FILEPATH)).unwrap();
        Self {
            discord_client_id: None, 
            discord_secret_key: None,
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
            let config_file = appdata_path.join("ds-config");
            if !Path::new(&config_file).exists() {
                self.save();
            } else {
                match File::open(config_file) {
                    Ok(mut file) => {
                        let mut nonce_bytes = [0u8; 12];
                        file.read_exact(&mut nonce_bytes)
                            .expect("Failed to read nonce");
                        let nonce = Nonce::from_slice(&nonce_bytes);

                        let mut ciphertext = Vec::new();
                        file.read_to_end(&mut ciphertext)
                            .expect("Failed to read ciphertext");

                        let key_hex =
                            var("ENCRYPTION_KEY").expect("Missing ENCRYPTION_KEY in environment");
                        let key_bytes =
                            decode(key_hex).expect("Invalid hex format in ENCRYPTION_KEY");
                        let key: [u8; 32] =
                            key_bytes.try_into().expect("Key must be exactly 32 bytes");

                        let cipher =
                            Aes256Gcm::new_from_slice(&key).expect("Failed to create cipher");
                        let decrypted = cipher
                            .decrypt(nonce, ciphertext.as_ref())
                            .expect("Decryption failed");

                        *self = serde_json::from_slice(&decrypted).expect("Failed to parse JSON");

                        if self.discord_client_id.is_none(){
                            self.discord_client_id = Some(var("DISCORD_CLIENT_ID").unwrap_or("".to_string()));
                        }
                        if self.discord_secret_key.is_none() {
                            self.discord_secret_key = Some(var("DISCORD_SECRET_KEY").unwrap_or("".to_string()));
                        }
                    }
                    Err(_) => {
                        self.save();
                    }
                };
            }
        }
    }

    pub fn save(&mut self) {
        if let Some(base_dirs) = BaseDirs::new() {
            let appdata_path = base_dirs.config_dir().join("Mechardo");
            if !appdata_path.exists() {
                std::fs::create_dir_all(&appdata_path).unwrap();
            }

            let json = serde_json::to_string_pretty(self).unwrap();

            let key_hex = var("ENCRYPTION_KEY").expect("Missing ENCRYPTION_KEY in environment");
            let key_bytes = decode(key_hex).expect("Invalid hex format in ENCRYPTION_KEY");
            let key: [u8; 32] = key_bytes.try_into().expect("Key must be exactly 32 bytes");
            let cipher = Aes256Gcm::new_from_slice(&key).expect("Failed to create cipher");

            let mut nonce_bytes = [0u8; 12];
            rand::thread_rng().fill_bytes(&mut nonce_bytes);
            let nonce = Nonce::from_slice(&nonce_bytes);

            let ciphertext = cipher
                .encrypt(nonce, json.as_bytes())
                .expect("Encryption failed");
            let file_path = appdata_path.join("ds-config");
            let mut file = File::create(file_path).expect("Cannot create file");
            file.write_all(&nonce_bytes).unwrap();
            file.write_all(&ciphertext).unwrap();
        }
    }
}
