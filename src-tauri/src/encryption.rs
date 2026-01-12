// Encryption module - Ready for future encryption feature implementation
// Currently unused but kept for when encryption support is added
#![allow(dead_code)]

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::Rng;
use sha2::{Sha256, Digest};
use anyhow::Result;

pub struct Encryptor {
    cipher: Aes256Gcm,
}

impl Encryptor {
    pub fn new(password: &str) -> Self {
        // Derive key from password
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        let key = hasher.finalize();
        
        let cipher = Aes256Gcm::new(&key);
        
        Self { cipher }
    }

    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        // Generate random nonce
        let mut rng = rand::thread_rng();
        let nonce_bytes: [u8; 12] = rng.gen();
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = self.cipher.encrypt(nonce, data)
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

        // Prepend nonce to ciphertext
        let mut result = nonce_bytes.to_vec();
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 12 {
            return Err(anyhow::anyhow!("Invalid encrypted data"));
        }

        // Extract nonce and ciphertext
        let nonce = Nonce::from_slice(&data[..12]);
        let ciphertext = &data[12..];

        // Decrypt
        let plaintext = self.cipher.decrypt(nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;

        Ok(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_decryption() {
        let encryptor = Encryptor::new("test_password");
        let data = b"Hello, World!";
        
        let encrypted = encryptor.encrypt(data).unwrap();
        let decrypted = encryptor.decrypt(&encrypted).unwrap();
        
        assert_eq!(data.to_vec(), decrypted);
    }
}
