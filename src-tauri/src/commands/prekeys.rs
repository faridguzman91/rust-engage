// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91>

// @faridguzman91: Tauri commands for one-time prekey (OPK) pool management.
//
// One-time prekeys give forward secrecy to the first message in an X3DH session:
// each new session from a different sender consumes one OPK from the server pool.
// When the pool drops below LOW_WATERMARK, the frontend calls generate_and_store_opks
// to generate a fresh batch and upload the public halves to the server.
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rusqlite::params;
use serde::Serialize;
use tauri::State;

use crate::crypto::keys::generate_one_time_prekeys;
use crate::AppState;

// @faridguzman91: These constants are mirrored in useOpkReplenishment.ts —
// keep them in sync if you change the thresholds.
pub const LOW_WATERMARK: u32 = 10;
pub const BATCH_SIZE: usize = 100;

#[derive(Serialize)]
pub struct OpkStatus {
    /// Number of unused OPKs currently held by the server for this device.
    pub server_remaining: u32,
    /// Whether the client should upload a fresh batch now.
    pub needs_replenishment: bool,
}

/// @faridguzman91: Stateless threshold check — accepts the server count from the
/// frontend so the decision logic lives in one place (Rust) rather than duplicated.
#[tauri::command]
pub fn get_opk_status(server_remaining: u32) -> OpkStatus {
    OpkStatus {
        server_remaining,
        needs_replenishment: server_remaining < LOW_WATERMARK,
    }
}

#[derive(Serialize)]
pub struct GeneratedOpks {
    /// Public halves — upload these to the server.
    pub public_keys: Vec<OpkPublic>,
    /// How many keys were generated and stored locally.
    pub count: usize,
}

#[derive(Serialize)]
pub struct OpkPublic {
    #[serde(rename = "keyId")]
    pub key_id: u32,
    #[serde(rename = "publicKey")]
    pub public_key: String,
}

/// @faridguzman91: Generate BATCH_SIZE fresh X25519 key pairs.
/// Private halves are stored in local SQLite (never uploaded).
/// Public halves are returned to the frontend for uploading to the server.
/// Key IDs start from max(existing) + 1 to avoid collisions.
#[tauri::command]
pub fn generate_and_store_opks(state: State<AppState>) -> Result<GeneratedOpks, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

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

        public_keys.push(OpkPublic { key_id, public_key: kp.public_key.clone() });
    }

    Ok(GeneratedOpks { count: public_keys.len(), public_keys })
}
