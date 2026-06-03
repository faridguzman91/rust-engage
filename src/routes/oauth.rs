use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use chrono::Utc;
use rand::distributions::Alphanumeric;
use rand::Rng;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use base64::Engine as _;
use crate::auth::issue_jwt;
use crate::state::AppState;

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const STATE_TTL_SECS: i64 = 300; // 5 minutes

fn random_state() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

/// GET /api/auth/google  — redirect the browser to Google's consent screen
pub async fn start(State(state): State<AppState>) -> impl IntoResponse {
    let csrf_state = random_state();

    {
        let db = state.db.lock().unwrap();
        let now = Utc::now().timestamp();
        // Prune stale states while we're here
        let _ = db.execute(
            "DELETE FROM oauth_states WHERE created_at < ?1",
            params![now - STATE_TTL_SECS],
        );
        db.execute(
            "INSERT INTO oauth_states (state, created_at) VALUES (?1, ?2)",
            params![csrf_state, now],
        )
        .unwrap();
    }

    let url = format!(
        "{GOOGLE_AUTH_URL}?client_id={}&redirect_uri={}&response_type=code\
         &scope=openid%20email%20profile&state={}&access_type=offline&prompt=consent",
        urlencoding::encode(&state.oauth.client_id),
        urlencoding::encode(&state.oauth.redirect_uri),
        csrf_state,
    );

    Redirect::temporary(&url)
}

#[derive(Deserialize)]
pub struct CallbackParams {
    pub code: String,
    pub state: String,
    pub error: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    id_token: String,
}

#[derive(Deserialize)]
struct GoogleUser {
    sub: String,
    email: String,
    name: Option<String>,
}

/// Decode the payload of a Google id_token JWT without signature verification.
/// We trust the token because we just received it from Google's token endpoint
/// over TLS using our own client_secret — no need for a separate userinfo call.
fn decode_id_token(id_token: &str) -> Result<GoogleUser, String> {
    let payload = id_token
        .split('.')
        .nth(1)
        .ok_or("malformed id_token: missing payload section")?;

    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|e| format!("base64 decode error: {e}"))?;

    serde_json::from_slice(&decoded).map_err(|e| format!("JSON decode error: {e}"))
}

/// GET /api/auth/google/callback  — Google redirects here after consent
pub async fn callback(
    State(state): State<AppState>,
    Query(params): Query<CallbackParams>,
) -> impl IntoResponse {
    if let Some(err) = params.error {
        return (StatusCode::BAD_REQUEST, format!("OAuth error: {err}")).into_response();
    }

    // Validate CSRF state
    {
        let db = state.db.lock().unwrap();
        let now = Utc::now().timestamp();
        let valid: bool = db
            .query_row(
                "SELECT 1 FROM oauth_states WHERE state=?1 AND created_at > ?2",
                params![params.state, now - STATE_TTL_SECS],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if !valid {
            return (StatusCode::BAD_REQUEST, "invalid or expired state — please try signing in again").into_response();
        }
        // Don't delete the state yet — only remove it after the full flow succeeds
        // so the user can retry if the token exchange fails
    }

    // Exchange code for tokens
    let token_res: TokenResponse = match reqwest::Client::new()
        .post(GOOGLE_TOKEN_URL)
        .form(&[
            ("code", params.code.as_str()),
            ("client_id", state.oauth.client_id.as_str()),
            ("client_secret", state.oauth.client_secret.as_str()),
            ("redirect_uri", state.oauth.redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
    {
        Ok(r) => match r.json::<TokenResponse>().await {
            Ok(t) => t,
            Err(e) => return (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
        },
        Err(e) => return (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    };

    // Decode user info directly from the id_token JWT payload —
    // no extra HTTP call needed; we trust the token because it came from
    // Google's token endpoint using our client_secret over TLS.
    let user: GoogleUser = match decode_id_token(&token_res.id_token) {
        Ok(u) => u,
        Err(e) => return (StatusCode::BAD_GATEWAY, format!("id_token decode failed: {e}")).into_response(),
    };

    // Look up or create user_id for this Google account
    let user_id: String = {
        let db = state.db.lock().unwrap();

        // Check if this Google account is already linked
        let existing: Option<String> = db
            .query_row(
                "SELECT user_id FROM oauth_accounts WHERE google_sub=?1",
                params![user.sub],
                |row| row.get(0),
            )
            .ok();

        if let Some(uid) = existing {
            uid
        } else {
            // New Google login — create a placeholder device entry so the user
            // can then register their crypto keys. Use google_sub as user_id.
            let uid = user.sub.clone();
            let now = Utc::now().timestamp();
            let display = user.name.as_deref().unwrap_or(&user.email);

            // Insert a minimal device row (crypto keys will be uploaded after app opens)
            let _ = db.execute(
                "INSERT OR IGNORE INTO devices
                 (user_id, display_name, identity_key, spk_public, spk_signature, reg_id, registered_at)
                 VALUES (?1,?2,'','','',0,?3)",
                params![uid, display, now],
            );

            db.execute(
                "INSERT OR IGNORE INTO oauth_accounts (google_sub, user_id, email) VALUES (?1,?2,?3)",
                params![user.sub, uid, user.email],
            )
            .unwrap();

            uid
        }
    };

    // Issue JWT
    let token = match issue_jwt(&state.oauth.jwt_secret, &user_id, &user.email) {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };

    // Everything succeeded — now consume the CSRF state so it can't be replayed
    {
        let db = state.db.lock().unwrap();
        let _ = db.execute("DELETE FROM oauth_states WHERE state=?1", params![params.state]);
    }

    // Deep-link back into the Tauri app
    Redirect::temporary(&format!("engage://auth?token={token}")).into_response()
}
