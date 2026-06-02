use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use rusqlite::params;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::models::*;
use crate::state::AppState;

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// POST /api/messages  — send an encrypted message
pub async fn send_message(
    State(state): State<AppState>,
    Json(req): Json<SendMessageRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let id = Uuid::new_v4().to_string();
    let ts = now_ms();

    // Infer sender_id from the context (in production this comes from auth token)
    // For now we use the sender_ik as identity (first 12 chars for brevity as placeholder)
    let sender_id = req.sender_ik[..req.sender_ik.len().min(16)].to_string();

    let stored = StoredMessage {
        id: id.clone(),
        sender_id: sender_id.clone(),
        sender_ik: req.sender_ik.clone(),
        ephemeral_key: req.ephemeral_key.clone(),
        otpk_id: req.otpk_id,
        ciphertext: req.ciphertext.clone(),
        timestamp: ts,
    };

    {
        let db = state.db.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        db.execute(
            "INSERT INTO messages
             (id, recipient_id, sender_id, sender_ik, ephemeral_key, otpk_id, ciphertext, timestamp)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                id,
                req.recipient_id,
                sender_id,
                req.sender_ik,
                req.ephemeral_key,
                req.otpk_id,
                req.ciphertext,
                ts
            ],
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    // If the recipient is online, push immediately via WebSocket
    if let Some(tx) = state.connections.get(&req.recipient_id) {
        let envelope = WsEnvelope::Message { payload: stored };
        if let Ok(json) = serde_json::to_string(&envelope) {
            let _ = tx.send(axum::extract::ws::Message::Text(json.into()));
        }
    }

    Ok(StatusCode::ACCEPTED)
}

/// GET /api/messages/:user_id  — fetch pending (undelivered) messages
pub async fn fetch_messages(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Json<Vec<StoredMessage>>, (StatusCode, String)> {
    let db = state.db.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut stmt = db
        .prepare(
            "SELECT id, sender_id, sender_ik, ephemeral_key, otpk_id, ciphertext, timestamp
             FROM messages WHERE recipient_id=?1 AND delivered=0 ORDER BY timestamp",
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let msgs: Vec<StoredMessage> = stmt
        .query_map(params![user_id], |row| {
            Ok(StoredMessage {
                id: row.get(0)?,
                sender_id: row.get(1)?,
                sender_ik: row.get(2)?,
                ephemeral_key: row.get(3)?,
                otpk_id: row.get(4)?,
                ciphertext: row.get(5)?,
                timestamp: row.get(6)?,
            })
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .collect::<Result<_, _>>()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Mark all as delivered
    db.execute(
        "UPDATE messages SET delivered=1 WHERE recipient_id=?1 AND delivered=0",
        params![user_id],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(msgs))
}
