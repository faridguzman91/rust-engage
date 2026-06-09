// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91>

// @faridguzman: Group chat routes — create groups, manage members, send group messages.
//
// Group message delivery:
//   The server fans out a group message to every member except the sender.
//   If a member is online (has a WS connection) the message is pushed immediately;
//   otherwise it is stored in the messages table with recipient_id = member_id and
//   a group_id column so the client knows it belongs to a group conversation.
use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use rusqlite::params;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::auth::Claims;
use crate::models::*;
use crate::routes::messages::next_seq;
use crate::state::AppState;

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// POST /api/groups — create a group and add initial members
pub async fn create_group(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<CreateGroupRequest>,
) -> Result<Json<Group>, (StatusCode, String)> {
    if req.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "group name cannot be empty".into()));
    }

    let group_id = Uuid::new_v4().to_string();
    let now = now_secs();
    let db = state.db.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    db.execute(
        "INSERT INTO groups (id, name, created_by, created_at) VALUES (?1,?2,?3,?4)",
        params![group_id, req.name.trim(), claims.sub, now],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Add the creator as the first member
    let (creator_display, creator_ik): (String, String) = db
        .query_row(
            "SELECT display_name, identity_key FROM devices WHERE user_id=?1",
            params![claims.sub],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| (StatusCode::NOT_FOUND, "creator device not registered".into()))?;

    db.execute(
        "INSERT INTO group_members (group_id, user_id, display_name, identity_key, joined_at)
         VALUES (?1,?2,?3,?4,?5)",
        params![group_id, claims.sub, creator_display, creator_ik, now],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Add each requested member (skip if not registered — they can be added later)
    for member_id in &req.members {
        if member_id == &claims.sub {
            continue; // already added as creator
        }
        let res: Result<(String, String), _> = db.query_row(
            "SELECT display_name, identity_key FROM devices WHERE user_id=?1",
            params![member_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );
        if let Ok((disp, ik)) = res {
            let _ = db.execute(
                "INSERT OR IGNORE INTO group_members (group_id, user_id, display_name, identity_key, joined_at)
                 VALUES (?1,?2,?3,?4,?5)",
                params![group_id, member_id, disp, ik, now],
            );
        }
    }

    let group = load_group(&db, &group_id, &claims.sub)?;
    Ok(Json(group))
}

/// GET /api/groups — list all groups the caller belongs to
pub async fn list_groups(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<Group>>, (StatusCode, String)> {
    let db = state.db.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut stmt = db
        .prepare("SELECT group_id FROM group_members WHERE user_id=?1")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let group_ids: Vec<String> = stmt
        .query_map(params![claims.sub], |row| row.get(0))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .collect::<Result<_, _>>()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut groups = Vec::new();
    for gid in &group_ids {
        if let Ok(g) = load_group(&db, gid, &claims.sub) {
            groups.push(g);
        }
    }
    Ok(Json(groups))
}

/// GET /api/groups/:id — get group info + full member list
pub async fn get_group(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(group_id): Path<String>,
) -> Result<Json<Group>, (StatusCode, String)> {
    let db = state.db.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    ensure_member(&db, &group_id, &claims.sub)?;
    let group = load_group(&db, &group_id, &claims.sub)?;
    Ok(Json(group))
}

/// POST /api/groups/:id/members — add a member (any existing member can add)
pub async fn add_member(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(group_id): Path<String>,
    Json(req): Json<AddMemberRequest>,
) -> Result<Json<GroupMember>, (StatusCode, String)> {
    let db = state.db.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    ensure_member(&db, &group_id, &claims.sub)?;

    let (disp, ik): (String, String) = db
        .query_row(
            "SELECT display_name, identity_key FROM devices WHERE user_id=?1",
            params![req.user_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| (StatusCode::NOT_FOUND, "user not registered".into()))?;

    db.execute(
        "INSERT OR IGNORE INTO group_members (group_id, user_id, display_name, identity_key, joined_at)
         VALUES (?1,?2,?3,?4,?5)",
        params![group_id, req.user_id, disp, ik, now_secs()],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(GroupMember { user_id: req.user_id, display_name: disp, identity_key: ik }))
}

/// DELETE /api/groups/:id/members/:user_id — leave or remove a member
pub async fn remove_member(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((group_id, user_id)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    // @faridguzman: Only the target user (leave) or the group creator (kick) may remove
    let db = state.db.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let creator: String = db
        .query_row("SELECT created_by FROM groups WHERE id=?1", params![group_id], |r| r.get(0))
        .map_err(|_| (StatusCode::NOT_FOUND, "group not found".into()))?;

    if claims.sub != user_id && claims.sub != creator {
        return Err((StatusCode::FORBIDDEN, "only the group creator can remove members".into()));
    }

    db.execute(
        "DELETE FROM group_members WHERE group_id=?1 AND user_id=?2",
        params![group_id, user_id],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/groups/:id/messages — send an encrypted group message (fan-out delivery)
pub async fn send_group_message(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(group_id): Path<String>,
    Json(req): Json<SendGroupMessageRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let ts = now_ms();
    let db = state.db.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    ensure_member(&db, &group_id, &claims.sub)?;

    // Load all member IDs except the sender
    let mut member_stmt = db
        .prepare("SELECT user_id FROM group_members WHERE group_id=?1 AND user_id!=?2")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let member_ids: Vec<String> = member_stmt
        .query_map(params![group_id, claims.sub], |row| row.get(0))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .collect::<Result<_, _>>()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // @faridguzman: Fan-out — store one row per recipient so each can fetch independently.
    // The ciphertext is the same for all (Sender Key encryption: one encrypt, N recipients).
    // Each recipient gets their own sequence number so they can detect delivery gaps.
    for member_id in &member_ids {
        let msg_id = Uuid::new_v4().to_string();

        // Assign next seq for this specific recipient before inserting
        let seq = next_seq(&db, member_id)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let stored = GroupStoredMessage {
            id: msg_id.clone(),
            group_id: group_id.clone(),
            sender_id: claims.sub.clone(),
            sender_ik: req.sender_ik.clone(),
            ciphertext: req.ciphertext.clone(),
            timestamp: ts,
            seq_num: Some(seq),
        };

        db.execute(
            "INSERT INTO messages
             (id, recipient_id, sender_id, sender_ik, ephemeral_key, otpk_id, ciphertext, timestamp, group_id, sequence_num)
             VALUES (?1,?2,?3,?4,NULL,NULL,?5,?6,?7,?8)",
            params![msg_id, member_id, claims.sub, req.sender_ik, req.ciphertext, ts, group_id, seq],
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Push to the member's WebSocket if they are online
        if let Some(tx) = state.connections.get(member_id.as_str()) {
            let envelope = WsEnvelope::GroupMessage { payload: stored };
            if let Ok(json) = serde_json::to_string(&envelope) {
                let _ = tx.send(axum::extract::ws::Message::Text(json.into()));
            }
        }
    }

    Ok(StatusCode::ACCEPTED)
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn load_group(
    db: &rusqlite::Connection,
    group_id: &str,
    caller_id: &str,
) -> Result<Group, (StatusCode, String)> {
    let (name, created_by, created_at): (String, String, i64) = db
        .query_row(
            "SELECT name, created_by, created_at FROM groups WHERE id=?1",
            params![group_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| (StatusCode::NOT_FOUND, "group not found".into()))?;

    let mut stmt = db
        .prepare(
            "SELECT user_id, display_name, identity_key FROM group_members WHERE group_id=?1",
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let members: Vec<GroupMember> = stmt
        .query_map(params![group_id], |row| {
            Ok(GroupMember {
                user_id: row.get(0)?,
                display_name: row.get(1)?,
                identity_key: row.get(2)?,
            })
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .collect::<Result<_, _>>()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Caller must be a member
    if !members.iter().any(|m| m.user_id == caller_id) {
        return Err((StatusCode::FORBIDDEN, "not a member of this group".into()));
    }

    Ok(Group { id: group_id.to_string(), name, created_by, created_at, members })
}

fn ensure_member(
    db: &rusqlite::Connection,
    group_id: &str,
    user_id: &str,
) -> Result<(), (StatusCode, String)> {
    let is_member: bool = db
        .query_row(
            "SELECT 1 FROM group_members WHERE group_id=?1 AND user_id=?2",
            params![group_id, user_id],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if !is_member {
        Err((StatusCode::FORBIDDEN, "not a member of this group".into()))
    } else {
        Ok(())
    }
}
