//! Simplified double-ratchet session (placeholder for full Signal protocol).
//! Replace with libsignal-client bindings for production.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

pub struct Session {
    send_key: [u8; 32],
    recv_key: [u8; 32],
}

impl Session {
    /// Derive a session from a shared ECDH secret (X3DH output).
    pub fn from_shared_secret(secret: &[u8], initiator: bool) -> Self {
        let hk = Hkdf::<Sha256>::new(None, secret);
        let mut okm = [0u8; 64];
        hk.expand(b"engage-v1-session", &mut okm).unwrap();
        let (a, b) = okm.split_at(32);
        let (send_key, recv_key): ([u8; 32], [u8; 32]) = if initiator {
            (a.try_into().unwrap(), b.try_into().unwrap())
        } else {
            (b.try_into().unwrap(), a.try_into().unwrap())
        };
        Self { send_key, recv_key }
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<String, String> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.send_key));
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| e.to_string())?;
        let mut out = nonce_bytes.to_vec();
        out.extend(ciphertext);
        Ok(B64.encode(out))
    }

    pub fn decrypt(&self, encoded: &str) -> Result<Vec<u8>, String> {
        let raw = B64.decode(encoded).map_err(|e| e.to_string())?;
        if raw.len() < 12 {
            return Err("ciphertext too short".into());
        }
        let (nonce_bytes, ct) = raw.split_at(12);
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.recv_key));
        let nonce = Nonce::from_slice(nonce_bytes);
        cipher.decrypt(nonce, ct).map_err(|e| e.to_string())
    }
}
