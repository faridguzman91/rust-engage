// @faridguzman: TURN credential issuance for WebRTC NAT traversal.
//
// Uses the standard coturn lt-cred-mechanism (long-term credential mechanism
// with short-term validity).  The same scheme is used by Signal, Twilio, and
// most WebRTC infrastructure providers.
//
// Credential derivation:
//   username   = "{expiry_unix}:{user_id}"     expiry = now + TTL
//   credential = base64( HMAC-SHA1(TURN_SECRET, username) )
//
// The TURN server verifies the credential by recomputing the HMAC — no state
// needed on the relay.  Credentials expire automatically after TTL seconds.
//
// coturn configuration required (turnserver.conf):
//   lt-cred-mechanism
//   use-auth-secret
//   static-auth-secret=<TURN_SECRET>
//   realm=engage.app
use axum::{extract::{Extension, State}, http::StatusCode, Json};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha1::Sha1;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::auth::Claims;
use crate::state::AppState;

/// @faridguzman: TURN credential TTL — 24 hours matches invite token lifetime.
const TTL_SECS: u64 = 24 * 60 * 60;

/// @faridguzman: Public STUN server used for development when no TURN secret
/// is configured.  Always included even in production so nearby peers can
/// connect without going through TURN at all.
const STUN_URL: &str = "stun:stun.l.google.com:19302";

#[derive(Serialize)]
pub struct TurnCredentials {
    /// ICE server objects ready to pass directly to RTCPeerConfiguration.iceServers.
    #[serde(rename = "iceServers")]
    pub ice_servers: Vec<IceServer>,
}

#[derive(Serialize)]
pub struct IceServer {
    pub urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

/// GET /api/turn-credentials
///
/// Returns an iceServers array ready for RTCPeerConfiguration.
/// Always includes the public Google STUN server.
/// Adds a TURN server entry only when TURN_SECRET is configured.
pub async fn turn_credentials(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<TurnCredentials>, (StatusCode, String)> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let expiry = now + TTL_SECS;

    // @faridguzman: username encodes both the expiry and the user so the TURN
    // server can validate the token without any shared state.
    let username = format!("{expiry}:{}", claims.sub);

    let mut ice_servers = vec![
        IceServer {
            urls: vec![STUN_URL.to_string()],
            username: None,
            credential: None,
        },
    ];

    // @faridguzman: Only attach TURN server if the secret is configured.
    // In development (no TURN_SECRET) the app still works for LAN / same-
    // network calls; TURN is only needed for symmetric NAT traversal.
    if let Some(secret) = &state.oauth.turn_secret {
        type HmacSha1 = Hmac<Sha1>;
        let mut mac = HmacSha1::new_from_slice(secret.as_bytes())
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        mac.update(username.as_bytes());
        let credential = B64.encode(mac.finalize().into_bytes());

        ice_servers.push(IceServer {
            urls: vec![
                "turn:engage.app:3478".to_string(),
                "turn:engage.app:3478?transport=tcp".to_string(),
            ],
            username: Some(username),
            credential: Some(credential),
        });
    }

    Ok(Json(TurnCredentials { ice_servers }))
}
