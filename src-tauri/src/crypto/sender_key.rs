//! @faridguzman91: Sender Key protocol for group messaging.
//!
//! Each member of a group has a Sender Key — a symmetric key they use to encrypt
//! messages to the group. Other members hold a copy of each sender's key so they
//! can decrypt. This means:
//!   - One encryption per message regardless of group size (unlike N pairwise sessions)
//!   - The server receives one ciphertext and fans it out to all members
//!   - Keys ratchet forward with each message for forward secrecy
//!
//! Distribution:
//!   When a user joins a group (or first sends to it), they distribute their Sender Key
//!   to every other member via that member's existing pairwise ratchet session.
//!   This is a regular encrypted message containing the SenderKeyDistributionMessage.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use hkdf::Hkdf;
use rand::RngCore;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::AppState;

// ── Key structures ────────────────────────────────────────────────────────────

/// @faridguzman91: A Sender Key is a 32-byte secret shared with all group members.
/// It ratchets forward with each message: next_key = HKDF(current_key, "engage-sk-ratchet").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenderKeyRecord {
    pub group_id: String,
    pub user_id: String,       // whose key this is
    pub key_bytes: Vec<u8>,    // current 32-byte key
    pub iteration: u32,        // how many times the key has been ratcheted
}

/// Serialisable distribution message — encrypted via pairwise ratchet and sent
/// to each group member so they can decrypt future messages from this sender.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenderKeyDistribution {
    pub group_id: String,
    pub sender_id: String,
    pub key_bytes: Vec<u8>,
    pub iteration: u32,
}

// ── Ratchet ───────────────────────────────────────────────────────────────────

fn ratchet(key: &[u8; 32]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, key);
    let mut next = [0u8; 32];
    hk.expand(b"engage-sk-ratchet", &mut next).unwrap();
    next
}

// ── Encrypt / decrypt ─────────────────────────────────────────────────────────

/// @faridguzman91: Encrypt a group message with our own Sender Key, then advance the key.
/// Returns (base64 ciphertext, updated record).
pub fn encrypt_group(
    record: &mut SenderKeyRecord,
    plaintext: &[u8],
) -> Result<String, String> {
    let key_arr: [u8; 32] = record.key_bytes[..32]
        .try_into()
        .map_err(|_| "bad key length")?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_arr));

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ct = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| e.to_string())?;

    let mut out = nonce_bytes.to_vec();
    out.extend(ct);

    // Advance the sender key
    let next = ratchet(&key_arr);
    record.key_bytes = next.to_vec();
    record.iteration += 1;

    Ok(B64.encode(out))
}

/// @faridguzman91: Decrypt a group message using the stored Sender Key for that sender.
/// The key must be at the correct iteration — forward-only, no key re-use.
pub fn decrypt_group(record: &SenderKeyRecord, ciphertext_b64: &str) -> Result<Vec<u8>, String> {
    let raw = B64.decode(ciphertext_b64).map_err(|e| e.to_string())?;
    if raw.len() < 12 {
        return Err("ciphertext too short".into());
    }
    let (nonce_bytes, ct) = raw.split_at(12);
    let key_arr: [u8; 32] = record.key_bytes[..32]
        .try_into()
        .map_err(|_| "bad key length")?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_arr));
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ct)
        .map_err(|e| format!("group decrypt failed: {e}"))
}

// ── Persistence helpers ───────────────────────────────────────────────────────

/// Load our own Sender Key for a group from SQLite, or generate one if missing.
pub fn get_or_create_sender_key(
    db: &rusqlite::Connection,
    group_id: &str,
    our_user_id: &str,
) -> Result<SenderKeyRecord, String> {
    let existing: Option<SenderKeyRecord> = db
        .query_row(
            "SELECT key_bytes, iteration FROM sender_keys
             WHERE group_id=?1 AND user_id=?2 AND is_ours=1",
            params![group_id, our_user_id],
            |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, u32>(1)?)),
        )
        .ok()
        .map(|(kb, iter)| SenderKeyRecord {
            group_id: group_id.to_string(),
            user_id: our_user_id.to_string(),
            key_bytes: kb,
            iteration: iter,
        });

    if let Some(record) = existing {
        return Ok(record);
    }

    // Generate a fresh Sender Key
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    let record = SenderKeyRecord {
        group_id: group_id.to_string(),
        user_id: our_user_id.to_string(),
        key_bytes: key.to_vec(),
        iteration: 0,
    };
    save_sender_key(db, &record, true)?;
    Ok(record)
}

pub fn save_sender_key(
    db: &rusqlite::Connection,
    record: &SenderKeyRecord,
    is_ours: bool,
) -> Result<(), String> {
    db.execute(
        "INSERT OR REPLACE INTO sender_keys
         (group_id, user_id, key_bytes, iteration, is_ours)
         VALUES (?1,?2,?3,?4,?5)",
        params![
            record.group_id,
            record.user_id,
            record.key_bytes,
            record.iteration,
            is_ours as i32,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_sender_key(
    db: &rusqlite::Connection,
    group_id: &str,
    user_id: &str,
) -> Option<SenderKeyRecord> {
    db.query_row(
        "SELECT key_bytes, iteration FROM sender_keys WHERE group_id=?1 AND user_id=?2",
        params![group_id, user_id],
        |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, u32>(1)?)),
    )
    .ok()
    .map(|(kb, iter)| SenderKeyRecord {
        group_id: group_id.to_string(),
        user_id: user_id.to_string(),
        key_bytes: kb,
        iteration: iter,
    })
}
