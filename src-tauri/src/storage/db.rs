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

        -- @faridguzman91: sessions stores the serialised RatchetState JSON per contact.
        CREATE TABLE IF NOT EXISTS sessions (
            contact_id  TEXT PRIMARY KEY,
            state_json  TEXT NOT NULL
        );

        -- @faridguzman91: Local OPK pool — private halves stored here, public halves uploaded.
        CREATE TABLE IF NOT EXISTS one_time_prekeys (
            key_id      INTEGER PRIMARY KEY,
            public_key  TEXT NOT NULL,
            private_key BLOB NOT NULL,
            used        INTEGER NOT NULL DEFAULT 0
        );

        -- @faridguzman91: Group metadata cached locally for display
        CREATE TABLE IF NOT EXISTS groups (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL,
            created_by  TEXT NOT NULL
        );

        -- @faridguzman91: Sender Keys for group messaging.
        -- is_ours=1 means this is the key we generate and distribute to others.
        -- is_ours=0 means it was received from another member and we use it to decrypt.
        CREATE TABLE IF NOT EXISTS sender_keys (
            group_id    TEXT NOT NULL,
            user_id     TEXT NOT NULL,
            key_bytes   BLOB NOT NULL,
            iteration   INTEGER NOT NULL DEFAULT 0,
            is_ours     INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (group_id, user_id)
        );
        ",
    )?;

    // @faridguzman91: Additive column migrations — ALTER TABLE IF NOT EXISTS isn't in SQLite,
    // so we inspect PRAGMA table_info and skip if the column is already present.
    // disappear_after_secs: per-conversation TTL in seconds (0 = disabled)
    // expires_at: Unix-ms timestamp when a message should be deleted (NULL = never)
    add_column_if_missing(conn, "contacts", "disappear_after_secs", "INTEGER NOT NULL DEFAULT 0")?;
    add_column_if_missing(conn, "messages",  "expires_at",           "INTEGER")?;

    Ok(())
}

/// @faridguzman91: Add a column to a table only if it doesn't already exist.
/// SQLite does not support IF NOT EXISTS on ALTER TABLE.
fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<()> {
    let exists: bool = conn
        .prepare(&format!("PRAGMA table_info({table})"))?
        .query_map([], |row| row.get::<_, String>(1))?
        .any(|name| name.map(|n| n == column).unwrap_or(false));

    if !exists {
        conn.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {definition};"
        ))?;
    }
    Ok(())
}
