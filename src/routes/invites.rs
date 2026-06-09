// @faridguzman: Invite link routes.
//
// Flow:
//   1. Authenticated user calls POST /api/invites → receives a short-lived token.
//   2. They share the link (engage://invite?token=TOKEN) via QR, copy, or email/SMS.
//   3. Recipient opens the app — deep link routes to InviteView.vue.
//   4. InviteView calls GET /api/invites/:token (public) → gets inviter's display name
//      and identity key bundle, then calls addContact locally.
//
// Tokens are 32 random bytes (hex-encoded, 64 chars), expire after 24 hours,
// and are single-use — redeemed atomically on GET.
use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::auth::Claims;
use crate::state::AppState;

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// @faridguzman: 24-hour token TTL in milliseconds.
const TOKEN_TTL_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Serialize)]
pub struct InviteCreated {
    pub token: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: i64,
    /// @faridguzman: Pre-built deep-link URL ready to share / encode as QR.
    pub url: String,
}

#[derive(Serialize)]
pub struct InviterBundle {
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "identityKey")]
    pub identity_key: String,
}

/// POST /api/invites — create a short-lived invite token for the caller.
/// Cleans up expired tokens for the user before inserting the new one so the
/// table does not accumulate stale rows.
pub async fn create_invite(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<InviteCreated>, (StatusCode, String)> {
    let db = state.db.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let now = now_ms();
    let expires_at = now + TOKEN_TTL_MS;

    // @faridguzman: Sweep expired tokens for this user to keep the table lean.
    db.execute(
        "DELETE FROM invite_tokens WHERE user_id=?1 AND expires_at < ?2",
        params![claims.sub, now],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // @faridguzman: Generate a 32-byte cryptographically random token using the
    // getrandom crate (same source of randomness used by the TLS stack).
    // Hex-encoded to 64 URL-safe ASCII characters.
    let mut raw = [0u8; 32];
    getrandom::fill(&mut raw)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let token: String = raw.iter().map(|b| format!("{b:02x}")).collect();

    db.execute(
        "INSERT INTO invite_tokens (token, user_id, created_at, expires_at, used)
         VALUES (?1, ?2, ?3, ?4, 0)",
        params![token, claims.sub, now, expires_at],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let url = format!("engage://invite?token={token}");
    Ok(Json(InviteCreated { token, expires_at, url }))
}

/// GET /api/invites/:token — public; redeem an invite token.
/// Returns the inviter's display name and identity key so the recipient can
/// call addContact locally.  Marks the token as used atomically.
pub async fn redeem_invite(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<InviterBundle>, (StatusCode, String)> {
    let db = state.db.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let now = now_ms();

    // @faridguzman: Look up the token — reject if missing, expired, or already used.
    let row: Option<(String, i64, i64)> = db
        .query_row(
            "SELECT user_id, expires_at, used FROM invite_tokens WHERE token=?1",
            params![token],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();

    let (user_id, expires_at, used) = row
        .ok_or((StatusCode::NOT_FOUND, "invite not found".into()))?;

    if used != 0 {
        return Err((StatusCode::GONE, "invite already used".into()));
    }
    if expires_at < now {
        return Err((StatusCode::GONE, "invite expired".into()));
    }

    // @faridguzman: Mark token used before returning the bundle so concurrent
    // requests on the same token cannot both succeed.
    db.execute(
        "UPDATE invite_tokens SET used=1 WHERE token=?1",
        params![token],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (display_name, identity_key): (String, String) = db
        .query_row(
            "SELECT display_name, identity_key FROM devices WHERE user_id=?1",
            params![user_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| (StatusCode::NOT_FOUND, "inviter not registered".into()))?;

    Ok(Json(InviterBundle { user_id, display_name, identity_key }))
}
