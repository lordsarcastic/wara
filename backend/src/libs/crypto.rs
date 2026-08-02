use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};

use crate::errors::wara::WaraError;

pub fn encrypt_secret(master_key: &str, plaintext: &str) -> String {
    let key = Sha256::digest(master_key.as_bytes());
    let encrypted: Vec<u8> = plaintext
        .as_bytes()
        .iter()
        .enumerate()
        .map(|(idx, byte)| byte ^ key[idx % key.len()])
        .collect();
    STANDARD.encode(encrypted)
}

pub fn decrypt_secret(master_key: &str, ciphertext: &str) -> Result<String, WaraError> {
    let bytes = STANDARD.decode(ciphertext)?;
    let key = Sha256::digest(master_key.as_bytes());
    let decrypted: Vec<u8> = bytes
        .iter()
        .enumerate()
        .map(|(idx, byte)| byte ^ key[idx % key.len()])
        .collect();
    Ok(String::from_utf8(decrypted)?)
}

pub fn redact(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    "********".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_secret() {
        let encrypted = encrypt_secret("key", "registry-token");
        assert_ne!(encrypted, "registry-token");
        assert_eq!(decrypt_secret("key", &encrypted).unwrap(), "registry-token");
    }
}
