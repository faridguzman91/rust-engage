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
            _ => {}
        }
    }

    state.connections.remove(&user_id);
    send_task.abort();
    info!("WS disconnected: {user_id}");
}
