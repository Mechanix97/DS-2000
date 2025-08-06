use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use directories::BaseDirs;
use hex::decode;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::env::var;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{self, AtomicBool};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};
use tracing::debug;

const DEFAULT_REDIRECT_URL: &str = "https://www.mechardo3d.xyz/";
const CONFIG_SAVE_INTERVAL: u64 = 60;

pub struct Config {
    inner: Arc<Mutex<ConfigInfo>>,
    refresh: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ConfigInfo {
    pub discord_client_id: String,
    pub discord_secret_key: String,
    pub redirect_url: String,
    pub discord_access_token: Option<String>,
    pub discord_refresh_token: Option<String>,
    pub last_used_port: Option<String>,
}

impl Config {
    pub fn new() -> Self {
        // dotenvy::from_path(Path::new(ENV_FILEPATH)).unwrap();
        let config_info = ConfigInfo {
            discord_client_id: "".to_string(),
            discord_secret_key: "".to_string(),
            redirect_url: DEFAULT_REDIRECT_URL.to_string(),
            discord_access_token: None,
            discord_refresh_token: None,
            last_used_port: None,
        };

        Self {
            inner: Arc::new(Mutex::new(config_info)),
            refresh: Arc::new(AtomicBool::new(false)),
            join_handle: None,
        }
    }

    pub async fn start(&mut self) {
        debug!("starting config thread");
        let inner_clone = self.inner.clone();
        let refresh_clone = self.refresh.clone();

        let join_handle = tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(CONFIG_SAVE_INTERVAL)).await;
                if refresh_clone.load(atomic::Ordering::Relaxed) {
                    save_inner(inner_clone.clone()).await;
                    refresh_clone.store(false, atomic::Ordering::Relaxed);
                }
            }
        });
        self.join_handle = Some(join_handle);
    }

    pub async fn load(&mut self) {
        if let Some(base_dirs) = BaseDirs::new() {
            let appdata_path = base_dirs.config_dir().join("Mechardo");
            if !appdata_path.exists() {
                std::fs::create_dir_all(&appdata_path).unwrap();
                return;
            }
            let config_file = appdata_path.join("ds-config");
            if !Path::new(&config_file).exists() {
                self.save().await;
            } else {
                match File::open(config_file) {
                    Ok(mut file) => {
                        let mut lock = self.inner.lock().await;
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

                        *lock = serde_json::from_slice(&decrypted).expect("Failed to parse JSON");

                        lock.discord_client_id = var("DISCORD_CLIENT_ID").unwrap_or("".to_string());

                        lock.discord_secret_key =
                            var("DISCORD_SECRET_KEY").unwrap_or("".to_string());
                    }
                    Err(_) => {
                        self.save().await;
                    }
                };
            }
        }
    }

    pub async fn save(&self) {
        save_inner(self.inner.clone()).await;
    }

    pub async fn get_discord_client_id(&self) -> String {
        self.inner.lock().await.discord_client_id.clone()
    }

    pub async fn get_discord_secret_key(&self) -> String {
        self.inner.lock().await.discord_secret_key.clone()
    }

    pub async fn get_redirect_url(&self) -> String {
        self.inner.lock().await.redirect_url.clone()
    }

    pub async fn get_discord_access_token(&self) -> Option<String> {
        self.inner.lock().await.discord_access_token.clone()
    }

    pub async fn get_discord_refresh_token(&self) -> Option<String> {
        self.inner.lock().await.discord_refresh_token.clone()
    }

    pub async fn get_last_used_port(&self) -> Option<String> {
        self.inner.lock().await.last_used_port.clone()
    }

    pub async fn update_tokens(
        &mut self,
        access_token: Option<String>,
        refresh_token: Option<String>,
    ) {
        let mut lock = self.inner.lock().await;

        if lock.discord_access_token != access_token || lock.discord_refresh_token != refresh_token
        {
            lock.discord_access_token = access_token;
            lock.discord_refresh_token = refresh_token;
            self.refresh.store(true, atomic::Ordering::Relaxed);
        }
    }

    pub async fn update_last_used_port(&mut self, last_used_port: Option<String>) {
        let mut lock = self.inner.lock().await;

        if lock.last_used_port != last_used_port {
            lock.last_used_port = last_used_port;
            self.refresh.store(true, atomic::Ordering::Relaxed);
        }
    }
}

async fn save_inner(inner: Arc<Mutex<ConfigInfo>>) {
    debug!("Saving configuration to file");
    if let Some(base_dirs) = BaseDirs::new() {
        let appdata_path = base_dirs.config_dir().join("Mechardo");
        if !appdata_path.exists() {
            std::fs::create_dir_all(&appdata_path).unwrap();
        }

        let mut lock = inner.lock().await;

        lock.discord_client_id = var("DISCORD_CLIENT_ID").unwrap_or("".to_string());

        lock.discord_secret_key = var("DISCORD_SECRET_KEY").unwrap_or("".to_string());

        let json = serde_json::to_string_pretty(&*lock).unwrap();

        let key_hex = var("ENCRYPTION_KEY").expect("Missing ENCRYPTION_KEY in environment");
        let key_bytes = decode(key_hex).expect("Invalid hex format in ENCRYPTION_KEY");
        let key: [u8; 32] = key_bytes.try_into().expect("Key must be exactly 32 bytes");
        let cipher = Aes256Gcm::new_from_slice(&key).expect("Failed to create cipher");

        let mut nonce_bytes = [0u8; 12];
        rand::rng().fill_bytes(&mut nonce_bytes);
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
