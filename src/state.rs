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
