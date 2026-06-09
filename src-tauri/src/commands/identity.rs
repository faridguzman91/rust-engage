// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91>

use rusqlite::params;
use serde::Serialize;
use tauri::State;

use crate::AppState;
use crate::crypto::identity::IdentityBundle;

#[derive(Serialize)]
pub struct IdentityKeys {
    pub identity_public_key: String,
    pub signed_pre_key_public_key: String,
    pub registration_id: u32,
}

#[derive(Serialize)]
pub struct IdentityResult {
    pub keys: IdentityKeys,
    pub display_name: String,
}

#[tauri::command]
pub fn create_identity(
    display_name: String,
    state: State<AppState>,
) -> Result<serde_json::Value, String> {
    let bundle = IdentityBundle::generate();
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.execute(
        "INSERT OR REPLACE INTO identity
         (id, display_name, public_key, private_key, spk_public, spk_private, reg_id)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            display_name,
            bundle.identity_public_key,
            bundle.identity_private_key_bytes,
            bundle.signed_prekey.public_key,
            bundle.signed_prekey_private_bytes,
            bundle.registration_id,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "keys": {
            "identityPublicKey": bundle.identity_public_key,
            "signedPreKeyPublicKey": bundle.signed_prekey.public_key,
            "registrationId": bundle.registration_id,
        }
    }))
}

#[tauri::command]
pub fn get_identity(state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let result = db.query_row(
        "SELECT display_name, public_key, spk_public, reg_id FROM identity WHERE id=1",
        [],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, u32>(3)?,
            ))
        },
    );

    match result {
        Ok((name, ik, spk, reg_id)) => Ok(serde_json::json!({
            "displayName": name,
            "keys": {
                "identityPublicKey": ik,
                "signedPreKeyPublicKey": spk,
                "registrationId": reg_id,
            }
        })),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            Err("no_identity".into())
        }
        Err(e) => Err(e.to_string()),
    }
}
