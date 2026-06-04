use rusqlite::{Connection, Result};
use std::path::Path;

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        -- Registered devices
        CREATE TABLE IF NOT EXISTS devices (
            user_id         TEXT PRIMARY KEY,
            display_name    TEXT NOT NULL,
            identity_key    TEXT NOT NULL,   -- base64 X25519 public key (IK)
            spk_public      TEXT NOT NULL,   -- signed prekey public
            spk_signature   TEXT NOT NULL,   -- ed25519 signature of SPK
            reg_id          INTEGER NOT NULL,
            registered_at   INTEGER NOT NULL
        );

        -- One-time prekeys pool (OPKS)
        CREATE TABLE IF NOT EXISTS one_time_prekeys (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id     TEXT NOT NULL REFERENCES devices(user_id),
            key_id      INTEGER NOT NULL,
            public_key  TEXT NOT NULL,
            used        INTEGER NOT NULL DEFAULT 0,
            UNIQUE(user_id, key_id)
        );

        -- Sealed-envelope message store (server sees only ciphertext)
        CREATE TABLE IF NOT EXISTS messages (
            id              TEXT PRIMARY KEY,
            recipient_id    TEXT NOT NULL,
            sender_id       TEXT NOT NULL,
            sender_ik       TEXT NOT NULL,   -- sender identity key (for first-message X3DH)
            ephemeral_key   TEXT,            -- X3DH ephemeral key (first message only)
            otpk_id         INTEGER,         -- one-time prekey consumed
            ciphertext      TEXT NOT NULL,   -- ratchet-encrypted payload (opaque to server)
            timestamp       INTEGER NOT NULL,
            delivered       INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_msg_recipient ON messages(recipient_id, delivered, timestamp);

        -- Google OAuth: maps google_sub → user_id (set after first OAuth login)
        CREATE TABLE IF NOT EXISTS oauth_accounts (
            google_sub       TEXT PRIMARY KEY,
            user_id          TEXT NOT NULL REFERENCES devices(user_id),
            email            TEXT NOT NULL,
            access_token     TEXT NOT NULL DEFAULT '',
            refresh_token    TEXT NOT NULL DEFAULT '',
            token_expires_at INTEGER NOT NULL DEFAULT 0
        );

        -- Short-lived CSRF state tokens for the OAuth flow
        CREATE TABLE IF NOT EXISTS oauth_states (
            state       TEXT PRIMARY KEY,
            created_at  INTEGER NOT NULL
        );

        -- @faridguzman91: Group chat tables
        CREATE TABLE IF NOT EXISTS groups (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL,
            created_by  TEXT NOT NULL REFERENCES devices(user_id),
            created_at  INTEGER NOT NULL
        );

        -- @faridguzman91: Group membership — one row per (group, member) pair.
        -- identity_key is denormalised here so members can distribute Sender Keys
        -- to each other without an extra prekey lookup.
        CREATE TABLE IF NOT EXISTS group_members (
            group_id     TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
            user_id      TEXT NOT NULL,
            display_name TEXT NOT NULL,
            identity_key TEXT NOT NULL,
            joined_at    INTEGER NOT NULL,
            PRIMARY KEY (group_id, user_id)
        );
        CREATE INDEX IF NOT EXISTS idx_gm_user ON group_members(user_id);
        ",
    )?;

    // @faridguzman91: Additive migration — add group_id to messages so group fan-out
    // rows can be distinguished from 1:1 messages on the client side.
    add_column_if_missing(conn, "messages", "group_id", "TEXT")?;
    add_column_if_missing(conn, "oauth_accounts", "access_token", "TEXT NOT NULL DEFAULT ''")?;
    add_column_if_missing(conn, "oauth_accounts", "refresh_token", "TEXT NOT NULL DEFAULT ''")?;
    add_column_if_missing(conn, "oauth_accounts", "token_expires_at", "INTEGER NOT NULL DEFAULT 0")?;

    Ok(())
}

fn add_column_if_missing(
    conn: &rusqlite::Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<()> {
    let exists: bool = conn
        .prepare(&format!("PRAGMA table_info({table})"))?
        .query_map([], |row| row.get::<_, String>(1))?
        .any(|n| n.map(|n| n == column).unwrap_or(false));
    if !exists {
        conn.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {definition};"
        ))?;
    }
    Ok(())
}
