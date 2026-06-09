// @faridguzman: Tauri commands for local message persistence.
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
            // @faridguzman: Exclude already-expired messages on load so stale rows
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

    // @faridguzman: Check if this conversation has a disappear timer active
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

// ── Pending message queue ─────────────────────────────────────────────────────

/// @faridguzman: A sealed envelope that failed to reach the relay server.
/// Stored verbatim so we can retry the POST without re-encrypting (the ratchet
/// has already advanced; re-encrypting would produce an out-of-order ciphertext
/// that the recipient cannot decrypt).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PendingMessage {
    pub id: String,
    #[serde(rename = "conversationId")]
    pub conversation_id: String,
    #[serde(rename = "recipientId")]
    pub recipient_id: String,
    #[serde(rename = "senderIk")]
    pub sender_ik: String,
    #[serde(rename = "ephemeralKey")]
    pub ephemeral_key: Option<String>,
    pub ciphertext: String,
    pub timestamp: i64,
    #[serde(rename = "retryCount")]
    pub retry_count: i32,
}

/// @faridguzman: Enqueue a sealed envelope that failed to send.
/// Called immediately after encrypt_message so the ciphertext is never lost,
/// even if the app crashes before the POST succeeds.
#[tauri::command]
pub fn queue_pending_message(
    message_id: String,
    conversation_id: String,
    recipient_id: String,
    sender_ik: String,
    ephemeral_key: Option<String>,
    ciphertext: String,
    timestamp: i64,
    state: State<AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute(
        "INSERT OR REPLACE INTO pending_messages
         (id, conversation_id, recipient_id, sender_ik, ephemeral_key, ciphertext, timestamp)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            message_id, conversation_id, recipient_id,
            sender_ik, ephemeral_key, ciphertext, timestamp
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// @faridguzman: Return all queued envelopes ordered by timestamp (oldest first).
/// The drain loop in the client iterates this list and retries each POST.
#[tauri::command]
pub fn list_pending_messages(state: State<AppState>) -> Result<Vec<PendingMessage>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db
        .prepare(
            "SELECT id, conversation_id, recipient_id, sender_ik,
                    ephemeral_key, ciphertext, timestamp, retry_count
             FROM pending_messages
             ORDER BY timestamp",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(PendingMessage {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                recipient_id: row.get(2)?,
                sender_ik: row.get(3)?,
                ephemeral_key: row.get(4)?,
                ciphertext: row.get(5)?,
                timestamp: row.get(6)?,
                retry_count: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    Ok(rows)
}

/// @faridguzman: Remove a successfully delivered envelope from the retry queue.
#[tauri::command]
pub fn remove_pending_message(
    message_id: String,
    state: State<AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute(
        "DELETE FROM pending_messages WHERE id=?1",
        params![message_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// @faridguzman: Increment the retry counter for a pending message so we can
/// surface persistent failures to the user after too many attempts.
#[tauri::command]
pub fn increment_pending_retry(
    message_id: String,
    state: State<AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute(
        "UPDATE pending_messages SET retry_count = retry_count + 1 WHERE id=?1",
        params![message_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
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
