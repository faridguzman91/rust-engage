// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91>

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
                    // @faridguzman: Pass user_id so call signaling can stamp
                    // from_user_id without trusting the client to send it.
                    handle_client_frame(frame, &user_id, &state).await;
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
/// Handles messaging receipts (ack/read) and WebRTC call signaling.
/// `caller_id` is the authenticated user_id of the sending connection.
async fn handle_client_frame(frame: ClientFrame, caller_id: &str, state: &AppState) {
    match frame.kind.as_str() {
        // ── Messaging receipts ────────────────────────────────────────────────
        "ack" | "read" => {
            let Some(message_id) = frame.message_id else { return };

            // @faridguzman: Look up original sender so we know where to forward
            // the receipt.  Lock held only for the DB query.
            let sender_id: Option<String> = state
                .db
                .lock()
                .ok()
                .and_then(|db| {
                    db.query_row(
                        "SELECT sender_id FROM messages WHERE id = ?1",
                        rusqlite::params![message_id],
                        |row| row.get::<_, String>(0),
                    )
                    .ok()
                });

            let Some(sid) = sender_id else { return };

            let envelope = if frame.kind == "ack" {
                WsEnvelope::Ack  { message_id }
            } else {
                WsEnvelope::Read { message_id }
            };

            push_to_peer(&state.connections, &sid, envelope);
        }

        // ── WebRTC call signaling ─────────────────────────────────────────────
        // @faridguzman: The server is a pure relay for all call frames — it
        // never inspects SDP or ICE candidates.  The `to` field addresses the
        // remote peer directly; no DB lookup is needed.
        "call_offer" => {
            let (Some(to), Some(call_id), Some(sdp)) =
                (frame.to, frame.call_id, frame.sdp) else { return };
            // @faridguzman: from_user_id is set by the server from the
            // authenticated WS connection — the callee cannot be spoofed.
            push_to_peer(
                &state.connections,
                &to,
                WsEnvelope::CallOffer {
                    call_id,
                    from_user_id: caller_id.to_string(),
                    sdp,
                    is_video: frame.is_video.unwrap_or(false),
                },
            );
        }

        "call_answer" => {
            let (Some(to), Some(call_id), Some(sdp)) =
                (frame.to, frame.call_id, frame.sdp) else { return };
            push_to_peer(&state.connections, &to, WsEnvelope::CallAnswer { call_id, sdp });
        }

        "ice_candidate" => {
            let (Some(to), Some(call_id), Some(candidate)) =
                (frame.to, frame.call_id, frame.candidate) else { return };
            push_to_peer(
                &state.connections,
                &to,
                WsEnvelope::IceCandidate {
                    call_id,
                    candidate,
                    sdp_mid: frame.sdp_mid,
                    sdp_m_line_index: frame.sdp_m_line_index,
                },
            );
        }

        "call_hangup" => {
            let (Some(to), Some(call_id)) = (frame.to, frame.call_id) else { return };
            push_to_peer(&state.connections, &to, WsEnvelope::CallHangup { call_id });
        }

        _ => {}
    }
}

/// @faridguzman: Serialise an envelope to JSON and deliver it to a connected peer.
/// Silently drops the message if the peer is not currently online.
fn push_to_peer(connections: &crate::state::Connections, peer_id: &str, envelope: WsEnvelope) {
    if let Ok(json) = serde_json::to_string(&envelope) {
        if let Some(tx) = connections.get(peer_id) {
            let _ = tx.send(Message::Text(json.into()));
        }
    }
}
