use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

use super::keys::generate_signed_prekey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityBundle {
    pub registration_id: u32,
    pub identity_public_key: String,
    #[serde(skip_serializing)]
    pub identity_private_key_bytes: Vec<u8>,
    pub signed_prekey: crate::crypto::keys::SignedPreKey,
    #[serde(skip_serializing)]
    pub signed_prekey_private_bytes: Vec<u8>,
}

impl IdentityBundle {
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let identity_public = B64.encode(signing_key.verifying_key().to_bytes());
        let reg_id: u32 = rand::random::<u16>() as u32;

        let (spk_kp, signed_prekey) = generate_signed_prekey(&signing_key, 1);

        Self {
            registration_id: reg_id,
            identity_public_key: identity_public,
            identity_private_key_bytes: signing_key.to_bytes().to_vec(),
            signed_prekey,
            signed_prekey_private_bytes: spk_kp.private_key_bytes,
        }
    }
}
