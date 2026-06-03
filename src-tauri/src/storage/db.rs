// @faridguzman91: SQLite database setup with WAL mode and auto-migration.
// All tables use CREATE TABLE IF NOT EXISTS so migrations are additive and
// safe to run on every startup without wiping existing data.
use rusqlite::{Connection, Result};
use std::path::Path;

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    // @faridguzman91: WAL mode gives better concurrent read performance and
    // foreign key enforcement catches referential integrity bugs at the DB layer.
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        -- @faridguzman91: Local identity — exactly one row (id=1).
        -- private_key and spk_private are stored as BLOBs (raw bytes, never base64 at rest).
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

        -- @faridguzman91: Messages store the encrypted body for outbound messages
        -- (so the UI can show sent status) and plaintext for inbound (after decrypt).
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

        -- @faridguzman91: sessions stores the serialised RatchetState JSON per contact.
        -- Persisting this to SQLite means sessions survive app restarts.
        CREATE TABLE IF NOT EXISTS sessions (
            contact_id  TEXT PRIMARY KEY,
            state_json  TEXT NOT NULL
        );

        -- @faridguzman91: Local OPK pool — private halves stored here,
        -- public halves uploaded to the server. used=1 after server claims it.
        CREATE TABLE IF NOT EXISTS one_time_prekeys (
            key_id      INTEGER PRIMARY KEY,
            public_key  TEXT NOT NULL,
            private_key BLOB NOT NULL,
            used        INTEGER NOT NULL DEFAULT 0
        );
        ",
    )
}
