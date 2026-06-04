use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use tauri::State;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::crypto::session::SessionManager;
use crate::crypto::x3dh::RemoteBundle;
use crate::AppState;

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

/// Returns our own prekey bundle to share with the key server.
#[tauri::command]
pub fn generate_prekey_bundle(state: State<AppState>) -> Result<PreKeyBundle, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let (ik_pub, spk_pub, reg_id) = db
        .query_row(
            "SELECT public_key, spk_public, reg_id FROM identity WHERE id=1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, u32>(2)?)),
        )
        .map_err(|e| e.to_string())?;

    // Fetch one unused one-time prekey if available
    let otpk = db
        .query_row(
            "SELECT key_id, public_key FROM one_time_prekeys WHERE used=0 LIMIT 1",
            [],
            |row| Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?)),
        )
        .ok()
        .map(|(kid, kpub)| OneTimePreKeyRef { key_id: kid, public_key: kpub });

    Ok(PreKeyBundle {
        registration_id: reg_id,
        identity_key: ik_pub,
        signed_prekey: SignedPreKeyRef {
            key_id: 1,
            public_key: spk_pub,
            signature: String::new(), // signature stored separately; relay server verifies
        },
        one_time_prekey: otpk,
    })
}

/// Establish an outbound session using the remote party's prekey bundle (X3DH initiator).
#[tauri::command]
pub fn init_session(
    contact_id: String,
    bundle: PreKeyBundle,
    state: State<AppState>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let (ik_priv_blob, spk_pub_str, spk_sig_str): (Vec<u8>, String, String) = db
        .query_row(
            "SELECT private_key, spk_public, '' FROM identity WHERE id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| e.to_string())?;

    let remote_ik_bytes = B64.decode(&bundle.identity_key).map_err(|e| e.to_string())?;
    let remote_spk_bytes = B64.decode(&bundle.signed_prekey.public_key).map_err(|e| e.to_string())?;
    let sig_bytes: [u8; 64] = if bundle.signed_prekey.signature.is_empty() {
        [0u8; 64]
    } else {
        B64.decode(&bundle.signed_prekey.signature)
            .map_err(|e| e.to_string())?
            .try_into()
            .map_err(|_| "bad signature length")?
    };

    let remote_ik_arr: [u8; 32] = remote_ik_bytes.try_into().map_err(|_| "bad IK length")?;
    let remote_spk_arr: [u8; 32] = remote_spk_bytes.try_into().map_err(|_| "bad SPK length")?;

    let otpk_pub = if let Some(ref otpk) = bundle.one_time_prekey {
        let b = B64.decode(&otpk.public_key).map_err(|e| e.to_string())?;
        let arr: [u8; 32] = b.try_into().map_err(|_| "bad OTPK length")?;
        Some(PublicKey::from(arr))
    } else {
        None
    };

    let remote_bundle = RemoteBundle {
        identity_key: PublicKey::from(remote_ik_arr),
        signed_prekey: PublicKey::from(remote_spk_arr),
        spk_signature: sig_bytes,
        one_time_prekey: otpk_pub,
    };

    let manager = SessionManager::new(&db);
    let ek_pub = manager.init_outbound_session(&contact_id, &ik_priv_blob, &remote_bundle)?;

    Ok(B64.encode(ek_pub.as_bytes()))
}

/// Encrypt a plaintext message for a contact with an established session.
#[tauri::command]
pub fn encrypt_message(
    contact_id: String,
    plaintext: String,
    state: State<AppState>,
) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let manager = SessionManager::new(&db);
    let msg_json = manager.encrypt(&contact_id, plaintext.as_bytes())?;
    Ok(serde_json::json!({
        "ciphertext": msg_json,
        "messageType": 1
    }))
}

/// Recipient: establish an inbound session from a first-message X3DH envelope.
/// Called by the frontend when it receives a WebSocket message that contains an ephemeralKey.
#[tauri::command]
pub fn init_inbound_session(
    contact_id: String,
    sender_ik: String,
    ephemeral_key: String,
    state: State<AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let (ik_priv_blob, spk_priv_blob): (Vec<u8>, Vec<u8>) = db
        .query_row(
            "SELECT private_key, spk_private FROM identity WHERE id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let sender_ik_bytes = B64.decode(&sender_ik).map_err(|e| e.to_string())?;
    let ek_bytes = B64.decode(&ephemeral_key).map_err(|e| e.to_string())?;

    let manager = SessionManager::new(&db);
    manager.init_inbound_session(
        &contact_id,
        &ik_priv_blob,
        &spk_priv_blob,
        &sender_ik_bytes,
        &ek_bytes,
    )
}

/// Decrypt a ratchet message from a contact.
#[tauri::command]
pub fn decrypt_message(
    contact_id: String,
    ciphertext: String,
    _message_type: u32,
    state: State<AppState>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let manager = SessionManager::new(&db);
    let pt = manager.decrypt(&contact_id, &ciphertext)?;
    String::from_utf8(pt).map_err(|e| e.to_string())
}
