use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::IntoResponse,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::models::WsEnvelope;
use crate::state::AppState;

/// GET /ws/:user_id  — WebSocket upgrade endpoint
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(user_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, user_id, state))
}

async fn handle_socket(socket: WebSocket, user_id: String, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    state.connections.insert(user_id.clone(), tx);
    info!("WS connected: {user_id}");

    // Spawn a task to forward outbound messages from the mpsc channel to the socket
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Read inbound messages (client ACKs, keepalives)
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                if let Ok(envelope) = serde_json::from_str::<serde_json::Value>(&text) {
                    if envelope.get("type").and_then(|t| t.as_str()) == Some("ack") {
                        // Client acknowledging a delivered message — nothing to do server-side
                        // (already marked delivered on fetch)
                    }
                }
            }
            Message::Close(_) => break,
            Message::Ping(payload) => {
                // pong is handled automatically by axum
                let _ = payload;
            }
            _ => {}
        }
    }

    state.connections.remove(&user_id);
    send_task.abort();
    info!("WS disconnected: {user_id}");
}
