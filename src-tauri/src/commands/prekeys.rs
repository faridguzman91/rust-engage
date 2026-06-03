use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rusqlite::params;
use serde::Serialize;
use tauri::State;

use crate::crypto::keys::generate_one_time_prekeys;
use crate::AppState;

pub const LOW_WATERMARK: u32 = 10;  // replenish when server pool falls below this
pub const BATCH_SIZE: usize = 100;  // number of OPKs to generate per replenishment

#[derive(Serialize)]
pub struct OpkStatus {
    /// Number of unused OPKs currently held by the server for this device.
    pub server_remaining: u32,
    /// Whether the client should upload a fresh batch now.
    pub needs_replenishment: bool,
}

/// Returns how many unused OPKs the server currently has for us, plus a
/// `needs_replenishment` flag the frontend uses to decide whether to upload.
#[tauri::command]
pub fn get_opk_status(server_remaining: u32) -> OpkStatus {
    OpkStatus {
        server_remaining,
        needs_replenishment: server_remaining < LOW_WATERMARK,
    }
}

#[derive(Serialize)]
pub struct GeneratedOpks {
    /// Public halves — send these to the server.
    pub public_keys: Vec<OpkPublic>,
    /// How many keys were stored locally (private halves in SQLite).
    pub count: usize,
}

#[derive(Serialize)]
pub struct OpkPublic {
    #[serde(rename = "keyId")]
    pub key_id: u32,
    #[serde(rename = "publicKey")]
    pub public_key: String,
}

/// Generate `BATCH_SIZE` fresh one-time prekeys, persist the private halves
/// locally, and return the public halves for uploading to the server.
#[tauri::command]
pub fn generate_and_store_opks(state: State<AppState>) -> Result<GeneratedOpks, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Find the highest existing key_id so we never reuse IDs
    let max_id: u32 = db
        .query_row(
            "SELECT COALESCE(MAX(key_id), 0) FROM one_time_prekeys",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let pairs = generate_one_time_prekeys(BATCH_SIZE);
    let mut public_keys = Vec::with_capacity(BATCH_SIZE);

    for (i, (kp, _)) in pairs.iter().enumerate() {
        let key_id = max_id + 1 + i as u32;
        db.execute(
            "INSERT OR IGNORE INTO one_time_prekeys (key_id, public_key, private_key, used)
             VALUES (?1, ?2, ?3, 0)",
            params![key_id, kp.public_key, kp.private_key_bytes],
        )
        .map_err(|e| e.to_string())?;

        public_keys.push(OpkPublic {
            key_id,
            public_key: kp.public_key.clone(),
        });
    }

    Ok(GeneratedOpks {
        count: public_keys.len(),
        public_keys,
    })
}
