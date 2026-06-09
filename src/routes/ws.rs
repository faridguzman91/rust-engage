use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;
use tracing::info;

use crate::auth::verify_jwt;
use crate::models::{ClientFrame, WsEnvelope};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct WsQuery {
    token: String,
}

/// GET /ws/:user_id?token=JWT
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(user_id): Path<String>,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Validate JWT before upgrading — WebSocket handshakes can't use Authorization headers
    match verify_jwt(&state.oauth.jwt_secret, &query.token) {
        Ok(claims) if claims.sub == user_id => {
            ws.on_upgrade(move |socket| handle_socket(socket, user_id, state))
        }
        _ => (StatusCode::UNAUTHORIZED, "invalid token").into_response(),
    }
}

async fn handle_socket(socket: WebSocket, user_id: String, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    state.connections.insert(user_id.clone(), tx);
    info!("WS connected: {user_id}");

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Close(_) => break,
            Message::Ping(p) => { let _ = p; }
            // Handle client → server control frames (ack, read)
            Message::Text(text) => {
                if let Ok(frame) = serde_json::from_str::<ClientFrame>(&text) {
                    handle_client_frame(frame, &state).await;
                }
            }
            _ => {}
        }
    }

    state.connections.remove(&user_id);
    send_task.abort();
    info!("WS disconnected: {user_id}");
}

/// Route a client-sent frame to the appropriate handler.
/// Currently handles "ack" and "read" — both look up the original sender
/// and forward the corresponding envelope to them if they are online.
async fn handle_client_frame(frame: ClientFrame, state: &AppState) {
    // Resolve the original sender for this message_id.
    // Lock is held only for the query, released before any async work.
    let sender_id: Option<String> = state
        .db
        .lock()
        .ok()
        .and_then(|db| {
            db.query_row(
                "SELECT sender_id FROM messages WHERE id = ?1",
                rusqlite::params![frame.message_id],
                |row| row.get::<_, String>(0),
            )
            .ok()
        });

    let Some(sid) = sender_id else { return };

    let envelope = match frame.kind.as_str() {
        "ack"  => WsEnvelope::Ack  { message_id: frame.message_id },
        "read" => WsEnvelope::Read { message_id: frame.message_id },
        _ => return,
    };

    if let Ok(json) = serde_json::to_string(&envelope) {
        if let Some(tx) = state.connections.get(&sid) {
            let _ = tx.send(Message::Text(json.into()));
        }
    }
}
