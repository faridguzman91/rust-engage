// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91>

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use rusqlite::params;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::auth::Claims;
use crate::models::*;
use crate::state::AppState;

/// @faridguzman: Atomically increment and return the next sequence number for
/// a recipient.  The Mutex<Connection> serialises all DB access so these two
/// operations are safe without an explicit transaction.
pub fn next_seq(db: &rusqlite::Connection, recipient_id: &str) -> rusqlite::Result<i64> {
    db.execute(
        "INSERT INTO seq_counters (recipient_id, last_seq) VALUES (?1, 1)
         ON CONFLICT(recipient_id) DO UPDATE SET last_seq = last_seq + 1",
        rusqlite::params![recipient_id],
    )?;
    db.query_row(
        "SELECT last_seq FROM seq_counters WHERE recipient_id = ?1",
        rusqlite::params![recipient_id],
        |r| r.get(0),
    )
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// POST /api/messages — send an encrypted envelope; sender_id comes from JWT, not request body
pub async fn send_message(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<SendMessageRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let id = Uuid::new_v4().to_string();
    let ts = now_ms();
    let sender_id = claims.sub.clone();

    // @faridguzman: Assign the next sequence number for this recipient before
    // storing or pushing — kept inside the Mutex lock so it's atomic.
    let seq_num = {
        let db = state.db.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let seq = next_seq(&db, &req.recipient_id)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        db.execute(
            "INSERT INTO messages
             (id, recipient_id, sender_id, sender_ik, ephemeral_key, otpk_id, ciphertext, timestamp, sequence_num)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                id, req.recipient_id, sender_id,
                req.sender_ik, req.ephemeral_key, req.otpk_id,
                req.ciphertext, ts, seq
            ],
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        seq
    };

    let stored = StoredMessage {
        id: id.clone(),
        sender_id: sender_id.clone(),
        sender_ik: req.sender_ik.clone(),
        ephemeral_key: req.ephemeral_key.clone(),
        otpk_id: req.otpk_id,
        ciphertext: req.ciphertext.clone(),
        timestamp: ts,
        seq_num: Some(seq_num),
    };

    if let Some(tx) = state.connections.get(&req.recipient_id) {
        let envelope = WsEnvelope::Message { payload: stored };
        if let Ok(json) = serde_json::to_string(&envelope) {
            let _ = tx.send(axum::extract::ws::Message::Text(json.into()));
        }
    }

    Ok(StatusCode::ACCEPTED)
}

/// GET /api/messages/:user_id — only the authenticated user can fetch their own messages
pub async fn fetch_messages(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(user_id): Path<String>,
) -> Result<Json<Vec<StoredMessage>>, (StatusCode, String)> {
    if claims.sub != user_id {
        return Err((StatusCode::FORBIDDEN, "cannot fetch messages for another user".into()));
    }

    let db = state.db.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut stmt = db
        .prepare(
            "SELECT id, sender_id, sender_ik, ephemeral_key, otpk_id, ciphertext, timestamp, group_id, sequence_num
             FROM messages WHERE recipient_id=?1 AND delivered=0 ORDER BY timestamp",
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let msgs: Vec<StoredMessage> = stmt
        .query_map(params![claims.sub], |row| {
            Ok(StoredMessage {
                id: row.get(0)?,
                sender_id: row.get(1)?,
                sender_ik: row.get(2)?,
                ephemeral_key: row.get(3)?,
                otpk_id: row.get(4)?,
                ciphertext: row.get(5)?,
                timestamp: row.get(6)?,
                // row 7 is group_id — skipped here; seq_num is row 8
                seq_num: row.get(8)?,
            })
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .collect::<Result<_, _>>()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    db.execute(
        "UPDATE messages SET delivered=1 WHERE recipient_id=?1 AND delivered=0",
        params![claims.sub],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(msgs))
}
