use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use x25519_dalek::{EphemeralSecret, PublicKey, StaticSecret};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyPair {
    pub public_key: String,
    #[serde(skip_serializing)]
    pub private_key_bytes: Vec<u8>,
}

impl KeyPair {
    pub fn generate_x25519() -> Self {
        let secret = StaticSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);
        Self {
            public_key: B64.encode(public.as_bytes()),
            private_key_bytes: secret.to_bytes().to_vec(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedPreKey {
    pub key_id: u32,
    pub public_key: String,
    pub signature: String,
}

pub fn generate_signed_prekey(signing_key: &SigningKey, key_id: u32) -> (KeyPair, SignedPreKey) {
    let kp = KeyPair::generate_x25519();
    let sig = signing_key.sign(kp.public_key.as_bytes());
    let spk = SignedPreKey {
        key_id,
        public_key: kp.public_key.clone(),
        signature: B64.encode(sig.to_bytes()),
    };
    (kp, spk)
}

pub fn generate_one_time_prekeys(count: usize) -> Vec<(KeyPair, u32)> {
    (0..count as u32).map(|id| (KeyPair::generate_x25519(), id)).collect()
}
