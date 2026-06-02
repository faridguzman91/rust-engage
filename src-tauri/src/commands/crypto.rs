use serde::{Deserialize, Serialize};
use tauri::State;

use crate::AppState;

#[derive(Serialize, Deserialize)]
pub struct PreKeyBundle {
    #[serde(rename = "registrationId")]
    pub registration_id: u32,
    #[serde(rename = "identityKey")]
    pub identity_key: String,
    #[serde(rename = "signedPreKey")]
    pub signed_prekey: SignedPreKeyRef,
    #[serde(rename = "oneTimePreKey")]
    pub one_time_prekey: Option<OneTimePreKeyRef>,
}

#[derive(Serialize, Deserialize)]
pub struct SignedPreKeyRef {
    #[serde(rename = "keyId")]
    pub key_id: u32,
    #[serde(rename = "publicKey")]
    pub public_key: String,
    pub signature: String,
}

#[derive(Serialize, Deserialize)]
pub struct OneTimePreKeyRef {
    #[serde(rename = "keyId")]
    pub key_id: u32,
    #[serde(rename = "publicKey")]
    pub public_key: String,
}

/// Returns our own prekey bundle to share with the key server.
#[tauri::command]
pub fn generate_prekey_bundle(state: State<AppState>) -> Result<PreKeyBundle, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let (ik, spk_pub, spk_sig, reg_id) = db
        .query_row(
            "SELECT public_key, spk_public, '', reg_id FROM identity WHERE id=1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, u32>(3)?,
                ))
            },
        )
        .map_err(|e| e.to_string())?;

    Ok(PreKeyBundle {
        registration_id: reg_id,
        identity_key: ik,
        signed_prekey: SignedPreKeyRef {
            key_id: 1,
            public_key: spk_pub,
            signature: spk_sig,
        },
        one_time_prekey: None,
    })
}

/// Placeholder — wire real Double Ratchet encryption here.
#[tauri::command]
pub fn encrypt_message(
    recipient_id: String,
    plaintext: String,
    _state: State<AppState>,
) -> Result<serde_json::Value, String> {
    // TODO: look up session for recipient_id, run ratchet encrypt
    Ok(serde_json::json!({
        "ciphertext": plaintext,
        "messageType": 1
    }))
}

/// Placeholder — wire real Double Ratchet decryption here.
#[tauri::command]
pub fn decrypt_message(
    sender_id: String,
    ciphertext: String,
    message_type: u32,
    _state: State<AppState>,
) -> Result<String, String> {
    // TODO: look up session for sender_id, run ratchet decrypt
    Ok(ciphertext)
}

/// Placeholder — initialise an X3DH session with a remote prekey bundle.
#[tauri::command]
pub fn init_session(
    recipient_id: String,
    bundle: PreKeyBundle,
    _state: State<AppState>,
) -> Result<(), String> {
    // TODO: run X3DH key agreement, store derived session keys
    Ok(())
}
