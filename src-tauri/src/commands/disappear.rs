// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91>

// @faridguzman91: Tauri commands for disappearing messages.
//
// Timer options (seconds):
//   0        = disabled
//   30       = 30 seconds
//   300      = 5 minutes
//   3600     = 1 hour
//   28800    = 8 hours
//   604800   = 1 week
//
// When a conversation has a non-zero timer:
//   - Outbound: expires_at is set at INSERT time (timestamp + disappear_after_secs * 1000)
//   - Inbound:  expires_at is set when the message is appended after decryption
//   - Sweep:    sweep_expired_messages deletes all rows where expires_at <= now()
//     (called on app start and periodically from the frontend)
use rusqlite::params;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

use crate::AppState;

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Get the disappear timer for a conversation (returns 0 if disabled).
#[tauri::command]
pub fn get_disappear_timer(
    contact_id: String,
    state: State<AppState>,
) -> Result<u64, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let secs: u64 = db
        .query_row(
            "SELECT disappear_after_secs FROM contacts WHERE id=?1",
            params![contact_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    Ok(secs)
}

/// Set the disappear timer for a conversation. Pass 0 to disable.
#[tauri::command]
pub fn set_disappear_timer(
    contact_id: String,
    secs: u64,
    state: State<AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute(
        "UPDATE contacts SET disappear_after_secs=?1 WHERE id=?2",
        params![secs, contact_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Set expires_at on a message that has already been inserted.
/// Called by the frontend when an inbound message is decrypted and the
/// conversation has an active disappear timer.
#[tauri::command]
pub fn set_message_expiry(
    message_id: String,
    expires_at: i64,
    state: State<AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute(
        "UPDATE messages SET expires_at=?1 WHERE id=?2",
        params![expires_at, message_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Serialize)]
pub struct SweptResult {
    pub deleted: usize,
}

/// Delete all messages whose expires_at has passed. Returns the count deleted.
/// Called on app start and on a 30-second interval from the frontend.
#[tauri::command]
pub fn sweep_expired_messages(state: State<AppState>) -> Result<SweptResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let deleted = db
        .execute(
            "DELETE FROM messages WHERE expires_at IS NOT NULL AND expires_at <= ?1",
            params![now_ms()],
        )
        .map_err(|e| e.to_string())?;
    Ok(SweptResult { deleted })
}
