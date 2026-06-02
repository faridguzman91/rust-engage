use axum::extract::ws::Message;
use dashmap::DashMap;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

pub type WsTx = mpsc::UnboundedSender<Message>;

/// Online users: user_id -> WebSocket sender channel
pub type Connections = Arc<DashMap<String, WsTx>>;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub connections: Connections,
}

impl AppState {
    pub fn new(conn: Connection) -> Self {
        Self {
            db: Arc::new(Mutex::new(conn)),
            connections: Arc::new(DashMap::new()),
        }
    }
}
