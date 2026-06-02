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
            "SELECT id, conversation_id, sender_id, body, timestamp, status, is_mine
             FROM messages WHERE conversation_id=?1 ORDER BY timestamp",
        )
        .map_err(|e| e.to_string())?;
    let msgs = stmt
        .query_map(params![conversation_id], |row| {
            Ok(Message {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                sender_id: row.get(2)?,
                body: row.get(3)?,
                timestamp: row.get(4)?,
                status: row.get(5)?,
                is_mine: row.get::<_, i32>(6)? != 0,
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

    // Encrypt with the ratchet session if one exists; otherwise store plaintext (pre-session state).
    let stored_body = {
        let manager = SessionManager::new(&db);
        match manager.encrypt(&conversation_id, body.as_bytes()) {
            Ok(ciphertext_json) => ciphertext_json,
            Err(_) => body.clone(), // no session yet — store plaintext until session is established
        }
    };

    let msg = Message {
        id: id.clone(),
        conversation_id: conversation_id.clone(),
        sender_id: "me".into(),
        body: body.clone(), // return plaintext to UI
        timestamp: ts,
        status: "sent".into(),
        is_mine: true,
    };

    db.execute(
        "INSERT INTO messages (id, conversation_id, sender_id, body, timestamp, status, is_mine)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![id, conversation_id, "me", stored_body, ts, "sent", 1],
    )
    .map_err(|e| e.to_string())?;

    Ok(msg)
}
