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
        CREATE TABLE IF NOT EXISTS identity (
            id          INTEGER PRIMARY KEY,
            display_name TEXT NOT NULL,
            public_key  TEXT NOT NULL,
            private_key BLOB NOT NULL,
            spk_public  TEXT NOT NULL,
            spk_private BLOB NOT NULL,
            reg_id      INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS contacts (
            id          TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            identity_key TEXT NOT NULL,
            last_seen   INTEGER
        );

        CREATE TABLE IF NOT EXISTS messages (
            id              TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            sender_id       TEXT NOT NULL,
            body            TEXT NOT NULL,
            timestamp       INTEGER NOT NULL,
            status          TEXT NOT NULL DEFAULT 'sent',
            is_mine         INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_messages_conv ON messages(conversation_id, timestamp);

        CREATE TABLE IF NOT EXISTS sessions (
            contact_id  TEXT PRIMARY KEY,
            state_json  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS one_time_prekeys (
            key_id      INTEGER PRIMARY KEY,
            public_key  TEXT NOT NULL,
            private_key BLOB NOT NULL,
            used        INTEGER NOT NULL DEFAULT 0
        );
        ",
    )
}
