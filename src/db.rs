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
            google_sub  TEXT PRIMARY KEY,
            user_id     TEXT NOT NULL REFERENCES devices(user_id),
            email       TEXT NOT NULL
        );

        -- Short-lived CSRF state tokens for the OAuth flow
        CREATE TABLE IF NOT EXISTS oauth_states (
            state       TEXT PRIMARY KEY,
            created_at  INTEGER NOT NULL
        );
        ",
    )
}
