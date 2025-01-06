use rand::Rng;
use rand::distributions::Alphanumeric;

pub fn generate_nonce(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()  
}
