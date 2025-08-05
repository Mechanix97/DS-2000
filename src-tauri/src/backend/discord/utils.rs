use rand::Rng;
use rand::distr::Alphanumeric;

pub fn generate_nonce(length: usize) -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}
