// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91>

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::AppState;

#[derive(Serialize, Deserialize, Debug)]
pub struct Contact {
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "identityPublicKey")]
    pub identity_public_key: String,
    #[serde(rename = "lastSeen")]
    pub last_seen: Option<i64>,
}

#[tauri::command]
pub fn list_contacts(state: State<AppState>) -> Result<Vec<Contact>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db
        .prepare("SELECT id, display_name, identity_key, last_seen FROM contacts ORDER BY display_name")
        .map_err(|e| e.to_string())?;
    let contacts = stmt
        .query_map([], |row| {
            Ok(Contact {
                id: row.get(0)?,
                display_name: row.get(1)?,
                identity_public_key: row.get(2)?,
                last_seen: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(contacts)
}

#[tauri::command]
pub fn add_contact(
    identity_key: String,
    display_name: String,
    state: State<AppState>,
) -> Result<Contact, String> {
    let id = Uuid::new_v4().to_string();
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute(
        "INSERT INTO contacts (id, display_name, identity_key) VALUES (?1, ?2, ?3)",
        params![id, display_name, identity_key],
    )
    .map_err(|e| e.to_string())?;
    Ok(Contact {
        id,
        display_name,
        identity_public_key: identity_key,
        last_seen: None,
    })
}

#[tauri::command]
pub fn remove_contact(id: String, state: State<AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute("DELETE FROM contacts WHERE id=?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}
