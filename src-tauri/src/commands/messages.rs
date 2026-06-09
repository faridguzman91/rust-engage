// @faridguzman91: Tauri commands for local message persistence.
// send_message respects the conversation's disappear timer — if disappear_after_secs > 0
// the row is inserted with expires_at = now + ttl so sweep_expired_messages removes it later.
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;
use uuid::Uuid;

use crate::crypto::session::SessionManager;
use crate::AppState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub id: String,
    #[serde(rename = "conversationId")]
    pub conversation_id: String,
    #[serde(rename = "senderId")]
    pub sender_id: String,
    pub body: String,
    pub timestamp: i64,
    pub status: String,
    #[serde(rename = "isMine")]
    pub is_mine: bool,
    #[serde(rename = "expiresAt")]
    pub expires_at: Option<i64>,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[tauri::command]
pub fn list_messages(
    conversation_id: String,
    state: State<AppState>,
) -> Result<Vec<Message>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db
        .prepare(
            // @faridguzman91: Exclude already-expired messages on load so stale rows
            // don't appear if sweep hasn't run yet this session.
            "SELECT id, conversation_id, sender_id, body, timestamp, status, is_mine, expires_at
             FROM messages
             WHERE conversation_id=?1
               AND (expires_at IS NULL OR expires_at > ?2)
             ORDER BY timestamp",
        )
        .map_err(|e| e.to_string())?;

    let now = now_ms();
    let msgs = stmt
        .query_map(params![conversation_id, now], |row| {
            Ok(Message {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                sender_id: row.get(2)?,
                body: row.get(3)?,
                timestamp: row.get(4)?,
                status: row.get(5)?,
                is_mine: row.get::<_, i32>(6)? != 0,
                expires_at: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(msgs)
}

#[tauri::command]
pub fn send_message(
    conversation_id: String,
    body: String,
    state: State<AppState>,
) -> Result<Message, String> {
    let id = Uuid::new_v4().to_string();
    let ts = now_ms();

    let db = state.db.lock().map_err(|e| e.to_string())?;

    // @faridguzman91: Check if this conversation has a disappear timer active
    let disappear_secs: u64 = db
        .query_row(
            "SELECT disappear_after_secs FROM contacts WHERE id=?1",
            params![conversation_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let expires_at: Option<i64> = if disappear_secs > 0 {
        Some(ts + (disappear_secs as i64 * 1000))
    } else {
        None
    };

    // Encrypt with the ratchet session if one exists; otherwise store plaintext.
    let stored_body = {
        let manager = SessionManager::new(&db);
        match manager.encrypt(&conversation_id, body.as_bytes()) {
            Ok(ct) => ct,
            Err(_) => body.clone(),
        }
    };

    let msg = Message {
        id: id.clone(),
        conversation_id: conversation_id.clone(),
        sender_id: "me".into(),
        body: body.clone(),
        timestamp: ts,
        status: "sent".into(),
        is_mine: true,
        expires_at,
    };

    db.execute(
        "INSERT INTO messages
         (id, conversation_id, sender_id, body, timestamp, status, is_mine, expires_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![id, conversation_id, "me", stored_body, ts, "sent", 1, expires_at],
    )
    .map_err(|e| e.to_string())?;

    Ok(msg)
}

/// Update the delivery status of a locally-stored message.
/// Called by the WS handler when the server forwards an ack (→ "delivered")
/// or a read receipt (→ "read") back to the original sender.
#[tauri::command]
pub fn update_message_status(
    message_id: String,
    status: String,
    state: State<AppState>,
) -> Result<(), String> {
    // Guard against unexpected status strings reaching the DB
    if !matches!(status.as_str(), "sending" | "sent" | "delivered" | "read" | "failed") {
        return Err(format!("unknown status: {status}"));
    }
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute(
        "UPDATE messages SET status=?1 WHERE id=?2 AND is_mine=1",
        params![status, message_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
