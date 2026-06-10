// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91>

// @faridguzman: Nextcloud OAuth 2.0 authentication routes.
//
// Setup (Nextcloud admin):
//   Settings → Security → OAuth 2.0 clients → Add client
//   Name:         engage
//   Redirect URI: http://localhost:3000/api/auth/nextcloud/callback  (dev)
//                 https://your-server.com/api/auth/nextcloud/callback (prod)
//
// Server env vars required to enable this provider:
//   NEXTCLOUD_URL            https://cloud.example.com
//   NEXTCLOUD_CLIENT_ID      <from Nextcloud OAuth app>
//   NEXTCLOUD_CLIENT_SECRET  <from Nextcloud OAuth app>
//   NEXTCLOUD_SERVER_NAME    "My Cloud"  (optional, shown on login button)
//   NEXTCLOUD_REDIRECT_URI   (optional, defaults to localhost:3000 callback)
//
// Once configured, /api/auth/providers returns nextcloud.enabled=true and the
// client shows a "Sign in with <server_name>" button on the login screen.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
    Json,
};
use chrono::Utc;
use rand::distributions::Alphanumeric;
use rand::Rng;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::auth::issue_jwt;
use crate::state::AppState;

const STATE_TTL_SECS: i64 = 300;

fn random_state() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

// ── /api/auth/providers ───────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ProvidersResponse {
    pub google: bool,
    pub nextcloud: NextcloudProviderInfo,
}

#[derive(Serialize)]
pub struct NextcloudProviderInfo {
    pub enabled: bool,
    /// Human-readable name for the login button, e.g. "My Cloud"
    #[serde(rename = "serverName")]
    pub server_name: String,
    /// Base URL shown as a hint in the UI
    #[serde(rename = "serverUrl")]
    pub server_url: String,
}

/// GET /api/auth/providers — public endpoint.
/// @faridguzman: Called by LoginView on mount so it can conditionally render
/// the Nextcloud button without hard-coding provider availability in the client.
pub async fn providers(State(state): State<AppState>) -> Json<ProvidersResponse> {
    let nc = state.oauth.nextcloud.as_ref();
    Json(ProvidersResponse {
        google: true,
        nextcloud: NextcloudProviderInfo {
            enabled: nc.is_some(),
            server_name: nc.map(|c| c.server_name.clone()).unwrap_or_default(),
            server_url:  nc.map(|c| c.server_url.clone()).unwrap_or_default(),
        },
    })
}

// ── /api/auth/nextcloud ───────────────────────────────────────────────────────

/// GET /api/auth/nextcloud — redirect to Nextcloud's OAuth consent screen.
pub async fn start(State(state): State<AppState>) -> impl IntoResponse {
    let Some(nc) = &state.oauth.nextcloud else {
        return (StatusCode::NOT_FOUND, "Nextcloud auth is not configured on this server").into_response();
    };

    let csrf_state = random_state();

    {
        let db = state.db.lock().unwrap();
        let now = Utc::now().timestamp();
        let _ = db.execute(
            "DELETE FROM oauth_states WHERE created_at < ?1",
            params![now - STATE_TTL_SECS],
        );
        db.execute(
            "INSERT INTO oauth_states (state, created_at) VALUES (?1, ?2)",
            params![csrf_state, now],
        ).unwrap();
    }

    // @faridguzman: Nextcloud OAuth 2.0 authorization endpoint.
    // scope must be empty string — Nextcloud ignores the scope param but
    // requires it to be present for the request to be accepted.
    let url = format!(
        "{}/index.php/apps/oauth2/authorize?response_type=code&client_id={}&redirect_uri={}&state={}",
        nc.server_url,
        urlencoding::encode(&nc.client_id),
        urlencoding::encode(&nc.redirect_uri),
        csrf_state,
    );

    Redirect::temporary(&url).into_response()
}

// ── /api/auth/nextcloud/callback ─────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CallbackParams {
    pub code: Option<String>,
    pub state: String,
    pub error: Option<String>,
}

#[derive(Deserialize)]
struct NcTokenResponse {
    access_token: String,
    #[allow(dead_code)]
    token_type: String,
}

/// @faridguzman: OCS API response for GET /ocs/v2.php/cloud/user
#[derive(Deserialize)]
struct NcOcsResponse {
    ocs: NcOcsData,
}

#[derive(Deserialize)]
struct NcOcsData {
    data: NcUserData,
}

#[derive(Deserialize)]
struct NcUserData {
    id: String,
    #[serde(rename = "display-name")]
    display_name: Option<String>,
    email: Option<String>,
}

/// GET /api/auth/nextcloud/callback — Nextcloud redirects here after consent.
pub async fn callback(
    State(state): State<AppState>,
    Query(params): Query<CallbackParams>,
) -> impl IntoResponse {
    if let Some(err) = params.error {
        return (StatusCode::BAD_REQUEST, format!("Nextcloud OAuth error: {err}")).into_response();
    }

    let Some(nc) = &state.oauth.nextcloud else {
        return (StatusCode::NOT_FOUND, "Nextcloud auth not configured").into_response();
    };

    let code = match params.code {
        Some(c) => c,
        None => return (StatusCode::BAD_REQUEST, "missing code parameter").into_response(),
    };

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
            return (StatusCode::BAD_REQUEST, "invalid or expired state — please try again").into_response();
        }
    }

    let http = reqwest::Client::new();

    // Exchange authorisation code for access token
    let token_url = format!("{}/index.php/apps/oauth2/api/v1/token", nc.server_url);
    let token_resp = match http
        .post(&token_url)
        .form(&[
            ("grant_type",    "authorization_code"),
            ("code",          code.as_str()),
            ("client_id",     nc.client_id.as_str()),
            ("client_secret", nc.client_secret.as_str()),
            ("redirect_uri",  nc.redirect_uri.as_str()),
        ])
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_GATEWAY, format!("token request failed: {e}")).into_response(),
    };

    if !token_resp.status().is_success() {
        let status = token_resp.status();
        let body = token_resp.text().await.unwrap_or_default();
        return (StatusCode::BAD_GATEWAY, format!("Nextcloud token error {status}: {body}")).into_response();
    }

    let nc_token: NcTokenResponse = match token_resp.json().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::BAD_GATEWAY, format!("token decode failed: {e}")).into_response(),
    };

    // @faridguzman: Fetch user identity from Nextcloud OCS API.
    // The OCS endpoint returns JSON when ?format=json is set.
    let user_url = format!("{}/ocs/v2.php/cloud/user?format=json", nc.server_url);
    let user_resp = match http
        .get(&user_url)
        .bearer_auth(&nc_token.access_token)
        .header("OCS-APIRequest", "true")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_GATEWAY, format!("user info request failed: {e}")).into_response(),
    };

    let nc_user: NcOcsResponse = match user_resp.json().await {
        Ok(u) => u,
        Err(e) => return (StatusCode::BAD_GATEWAY, format!("user info decode failed: {e}")).into_response(),
    };

    let nc_data = nc_user.ocs.data;

    // @faridguzman: Stable unique ID — host:username — so accounts on different
    // Nextcloud instances don't collide even if they share a username.
    let nc_host = url::Url::parse(&nc.server_url)
        .ok()
        .and_then(|u| u.host_str().map(String::from))
        .unwrap_or_else(|| nc.server_url.clone());
    let nc_user_id = format!("{}:{}", nc_host, nc_data.id);

    let display_name = nc_data.display_name
        .clone()
        .unwrap_or_else(|| nc_data.id.clone());
    let email = nc_data.email.clone().unwrap_or_default();

    // Look up or create the engage user_id for this Nextcloud account
    let user_id: String = {
        let db = state.db.lock().unwrap();

        let existing: Option<String> = db
            .query_row(
                "SELECT user_id FROM nextcloud_accounts WHERE nc_user_id=?1",
                params![nc_user_id],
                |row| row.get(0),
            )
            .ok();

        if let Some(uid) = existing {
            uid
        } else {
            // @faridguzman: New Nextcloud login — create a placeholder device row.
            // Prefix with "nc_" so Nextcloud-origin IDs are distinguishable from
            // Google-origin IDs in logs and the devices table.
            let uid = format!("nc_{}", uuid::Uuid::new_v4());
            let now = Utc::now().timestamp();

            let _ = db.execute(
                "INSERT OR IGNORE INTO devices
                 (user_id, display_name, identity_key, spk_public, spk_signature, reg_id, registered_at)
                 VALUES (?1,?2,'','','',0,?3)",
                params![uid, display_name, now],
            );

            db.execute(
                "INSERT OR IGNORE INTO nextcloud_accounts (nc_user_id, user_id, email)
                 VALUES (?1,?2,?3)",
                params![nc_user_id, uid, email],
            ).unwrap();

            uid
        }
    };

    // Issue engage JWT — same format as Google auth
    let token = match issue_jwt(&state.oauth.jwt_secret, &user_id, &email) {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };

    // Consume the CSRF state
    {
        let db = state.db.lock().unwrap();
        let _ = db.execute("DELETE FROM oauth_states WHERE state=?1", params![params.state]);
    }

    // @faridguzman: Redirect back to the app using the same pattern as Google auth.
    let redirect_url = match std::env::var("FRONTEND_URL") {
        Ok(url) => format!("{url}/#/auth?token={token}"),
        Err(_)  => format!("engage://auth?token={token}"),
    };

    Redirect::temporary(&redirect_url).into_response()
}
