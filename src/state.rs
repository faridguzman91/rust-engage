// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91>

use axum::extract::ws::Message;
use dashmap::DashMap;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

pub type WsTx = mpsc::UnboundedSender<Message>;
pub type Connections = Arc<DashMap<String, WsTx>>;

#[derive(Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub jwt_secret: String,
    /// @faridguzman: Shared secret for HMAC-SHA1 short-term TURN credentials.
    /// Set via TURN_SECRET env var. If absent, /api/turn-credentials returns
    /// only the public STUN server so development still works without coturn.
    pub turn_secret: Option<String>,
    /// @faridguzman: Optional Nextcloud OAuth provider.
    /// Set NEXTCLOUD_URL + NEXTCLOUD_CLIENT_ID + NEXTCLOUD_CLIENT_SECRET to enable.
    /// The admin of the Nextcloud instance must create an OAuth 2.0 app under
    /// Settings → Security → OAuth 2.0 and supply the client credentials here.
    pub nextcloud: Option<NextcloudConfig>,
}

/// @faridguzman: Configuration for a single Nextcloud instance acting as an
/// OAuth identity provider.  One engage deployment talks to one Nextcloud.
#[derive(Clone)]
pub struct NextcloudConfig {
    /// Base URL of the Nextcloud instance, e.g. https://cloud.example.com
    pub server_url: String,
    /// Human-readable name shown on the login button, e.g. "My Cloud"
    pub server_name: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub connections: Connections,
    pub oauth: OAuthConfig,
}

impl AppState {
    pub fn new(conn: Connection, oauth: OAuthConfig) -> Self {
        Self {
            db: Arc::new(Mutex::new(conn)),
            connections: Arc::new(DashMap::new()),
            oauth,
        }
    }
}
