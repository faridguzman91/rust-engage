// @faridguzman91: Tauri commands for group chat.
//
// Group message flow (send):
//   1. get_or_create_sender_key — load or generate our Sender Key for the group
//   2. encrypt_group_message    — AES-256-GCM encrypt with Sender Key, advance key
//   3. Frontend POSTs ciphertext to server (/api/groups/:id/messages)
//   4. Server fans out to all members
//
// Group message flow (receive):
//   1. WS delivers GroupMessage envelope with sender_id + ciphertext
//   2. Frontend calls decrypt_group_message with sender_id
//   3. Looks up that sender's Sender Key record in SQLite and decrypts
//
// Sender Key distribution:
//   When we first send to a group, distribute_sender_key encodes our Sender Key
//   as a JSON control message and encrypts it via the pairwise ratchet to each member.
//   Recipients call store_received_sender_key after decrypting the distribution message.
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::crypto::sender_key::{
    decrypt_group, encrypt_group, get_or_create_sender_key, load_sender_key,
    save_sender_key, SenderKeyDistribution, SenderKeyRecord,
};
use crate::crypto::session::SessionManager;
use crate::AppState;

#[derive(Serialize, Deserialize)]
pub struct GroupInfo {
    pub id: String,
    pub name: String,
    #[serde(rename = "createdBy")]
    pub created_by: String,
}

/// Persist a group received from the server into local SQLite for display.
#[tauri::command]
pub fn cache_group(
    id: String,
    name: String,
    created_by: String,
    state: State<AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute(
        "INSERT OR REPLACE INTO groups (id, name, created_by) VALUES (?1,?2,?3)",
        params![id, name, created_by],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// List cached groups.
#[tauri::command]
pub fn list_cached_groups(state: State<AppState>) -> Result<Vec<GroupInfo>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db
        .prepare("SELECT id, name, created_by FROM groups ORDER BY name")
        .map_err(|e| e.to_string())?;
    let groups = stmt
        .query_map([], |row| {
            Ok(GroupInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                created_by: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    Ok(groups)
}

/// @faridguzman91: Encrypt a group message using our Sender Key.
/// Advances the Sender Key by one ratchet step after encryption so the next message
/// uses a different key (forward secrecy within the group session).
#[tauri::command]
pub fn encrypt_group_message(
    group_id: String,
    our_user_id: String,
    plaintext: String,
    state: State<AppState>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut record = get_or_create_sender_key(&db, &group_id, &our_user_id)?;
    let ciphertext = encrypt_group(&mut record, plaintext.as_bytes())?;
    save_sender_key(&db, &record, true)?;
    Ok(ciphertext)
}

/// @faridguzman91: Decrypt a group message from another member using their Sender Key.
#[tauri::command]
pub fn decrypt_group_message(
    group_id: String,
    sender_id: String,
    ciphertext: String,
    state: State<AppState>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let record = load_sender_key(&db, &group_id, &sender_id)
        .ok_or_else(|| format!("no sender key for {sender_id} in group {group_id}"))?;
    let plaintext = decrypt_group(&record, &ciphertext)?;
    // @faridguzman91: Advance the received key after successful decryption
    let mut advanced = record.clone();
    let key_arr: [u8; 32] = advanced.key_bytes[..32].try_into().map_err(|_| "bad key")?;
    let next = {
        use hkdf::Hkdf;
        use sha2::Sha256;
        let hk = Hkdf::<Sha256>::new(None, &key_arr);
        let mut out = [0u8; 32];
        hk.expand(b"engage-sk-ratchet", &mut out).unwrap();
        out
    };
    advanced.key_bytes = next.to_vec();
    advanced.iteration += 1;
    save_sender_key(&db, &advanced, false)?;
    String::from_utf8(plaintext).map_err(|e| e.to_string())
}

/// @faridguzman91: Serialise our Sender Key as a distribution message so it can be
/// encrypted pairwise and sent to a group member. Call this for every member when
/// joining a group or rotating keys.
#[tauri::command]
pub fn get_sender_key_distribution(
    group_id: String,
    our_user_id: String,
    state: State<AppState>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let record = get_or_create_sender_key(&db, &group_id, &our_user_id)?;
    let dist = SenderKeyDistribution {
        group_id,
        sender_id: our_user_id,
        key_bytes: record.key_bytes,
        iteration: record.iteration,
    };
    serde_json::to_string(&dist).map_err(|e| e.to_string())
}

/// @faridguzman91: Store a Sender Key received from another group member.
/// The distribution_json comes from their pairwise-decrypted message.
#[tauri::command]
pub fn store_received_sender_key(
    distribution_json: String,
    state: State<AppState>,
) -> Result<(), String> {
    let dist: SenderKeyDistribution =
        serde_json::from_str(&distribution_json).map_err(|e| e.to_string())?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let record = SenderKeyRecord {
        group_id: dist.group_id,
        user_id: dist.sender_id,
        key_bytes: dist.key_bytes,
        iteration: dist.iteration,
    };
    save_sender_key(&db, &record, false)
}
