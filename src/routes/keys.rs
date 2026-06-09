// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91>

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use rusqlite::params;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::auth::Claims;
use crate::models::*;
use crate::state::AppState;

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// POST /api/register — upload crypto keys for the authenticated user
pub async fn register(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<RegisterRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // user_id is always taken from the JWT — client cannot spoof it
    db.execute(
        "INSERT OR REPLACE INTO devices
         (user_id, display_name, identity_key, spk_public, spk_signature, reg_id, registered_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            claims.sub,
            req.display_name,
            req.identity_key,
            req.signed_prekey.public_key,
            req.signed_prekey.signature,
            req.registration_id,
            now()
        ],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for otpk in &req.one_time_prekeys {
        db.execute(
            "INSERT OR IGNORE INTO one_time_prekeys (user_id, key_id, public_key) VALUES (?1,?2,?3)",
            params![claims.sub, otpk.key_id, otpk.public_key],
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(StatusCode::CREATED)
}

/// GET /api/keys/:user_id — fetch a prekey bundle (any authenticated user can fetch any bundle)
pub async fn get_prekey_bundle(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(user_id): Path<String>,
) -> Result<Json<PreKeyBundle>, (StatusCode, String)> {
    let db = state.db.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (identity_key, spk_pub, spk_sig, reg_id): (String, String, String, u32) = db
        .query_row(
            "SELECT identity_key, spk_public, spk_signature, reg_id FROM devices WHERE user_id=?1",
            params![user_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|_| (StatusCode::NOT_FOUND, "user not found".into()))?;

    let otpk = db
        .query_row(
            "SELECT id, key_id, public_key FROM one_time_prekeys
             WHERE user_id=?1 AND used=0 LIMIT 1",
            params![user_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, u32>(1)?, row.get::<_, String>(2)?)),
        )
        .ok()
        .and_then(|(row_id, key_id, pub_key)| {
            db.execute("UPDATE one_time_prekeys SET used=1 WHERE id=?1", params![row_id]).ok()?;
            Some(OneTimePreKey { key_id, public_key: pub_key })
        });

    Ok(Json(PreKeyBundle {
        registration_id: reg_id,
        identity_key,
        signed_prekey: SignedPreKey { key_id: 1, public_key: spk_pub, signature: spk_sig },
        one_time_prekey: otpk,
    }))
}

/// POST /api/keys/:user_id/prekeys — replenish OPKs (only owner can upload their own keys)
pub async fn upload_prekeys(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(user_id): Path<String>,
    Json(keys): Json<Vec<OneTimePreKey>>,
) -> Result<StatusCode, (StatusCode, String)> {
    if claims.sub != user_id {
        return Err((StatusCode::FORBIDDEN, "cannot upload keys for another user".into()));
    }
    let db = state.db.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    for otpk in &keys {
        db.execute(
            "INSERT OR IGNORE INTO one_time_prekeys (user_id, key_id, public_key) VALUES (?1,?2,?3)",
            params![claims.sub, otpk.key_id, otpk.public_key],
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/keys/:userId/prekeys/count — how many unused OPKs does the server hold?
/// Only the owning user can query their own count.
pub async fn opk_count(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(user_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if claims.sub != user_id {
        return Err((StatusCode::FORBIDDEN, "cannot query another user's prekey count".into()));
    }
    let db = state.db.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let count: u32 = db
        .query_row(
            "SELECT COUNT(*) FROM one_time_prekeys WHERE user_id=?1 AND used=0",
            params![claims.sub],
            |row| row.get(0),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({ "remaining": count })))
}
