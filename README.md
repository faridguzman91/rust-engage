# engage-server

Relay server for the [engage](https://github.com/faridguzman91/rust-engage) encrypted chat app.

The server is a **zero-knowledge forwarder** — it stores and delivers sealed envelopes but never holds decryption keys and cannot read message content. It also acts as the public key distribution point so clients can initiate X3DH sessions with each other.

---

## Architecture

```
Clients                        engage-server
───────                        ─────────────
POST /api/register       ──►   Store public keys (IK, SPK, OPKs)
GET  /api/keys/:id       ──►   Return prekey bundle (claims one OPK atomically)
POST /api/keys/:id/prekeys ►   Replenish one-time prekeys
POST /api/messages       ──►   Store ciphertext, push to recipient WS if online
GET  /api/messages/:id   ──►   Fetch + mark delivered (offline drain)
GET  /ws/:id             ──►   WebSocket — real-time delivery channel
```

All message payloads are opaque `ciphertext` blobs encrypted by the client. The server never attempts to decrypt them.

---

## Repository layout

```
engage-server/
├── src/
│   ├── main.rs          # Axum router, startup, env config
│   ├── db.rs            # SQLite schema + WAL-mode connection
│   ├── models.rs        # Request/response types + WsEnvelope
│   ├── state.rs         # Shared AppState (db + live WS connection map)
│   └── routes/
│       ├── keys.rs      # /api/register, /api/keys/:id, /api/keys/:id/prekeys
│       ├── messages.rs  # /api/messages (send + fetch)
│       └── ws.rs        # /ws/:user_id WebSocket handler
└── Cargo.toml
```

---

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Rust | ≥ 1.96 | Install via [rustup](https://rustup.rs) |
| C linker | — | **Windows:** MinGW GCC (`scoop install gcc`) or MSVC Build Tools. **macOS/Linux:** Xcode CLT / `build-essential` |

### Windows toolchain note

The project targets `x86_64-pc-windows-gnu` (set in `rust-toolchain.toml`). Ensure the GNU toolchain and GCC are installed:

```powershell
scoop install gcc
rustup toolchain install stable-x86_64-pc-windows-gnu
```

---

## Running

### Development

```bash
cargo run
```

Server starts on `http://0.0.0.0:3000` by default.

### Environment variables

| Variable | Default | Description |
|---|---|---|
| `PORT` | `3000` | TCP port to listen on |
| `DATABASE_PATH` | `engage-server.db` | Path to the SQLite database file |
| `RUST_LOG` | _(unset)_ | Log verbosity — e.g. `engage_server=debug,tower_http=info` |

```bash
PORT=8080 DATABASE_PATH=/data/engage.db RUST_LOG=info cargo run
```

### Release build

```bash
cargo build --release
./target/release/engage-server
```

---

## API reference

### `POST /api/register`

Register a new device and upload its public keys.

**Request body**
```json
{
  "userId": "string",
  "displayName": "string",
  "identityKey": "base64(IK_pub)",
  "signedPreKey": {
    "keyId": 1,
    "publicKey": "base64(SPK_pub)",
    "signature": "base64(Ed25519 sig of SPK_pub)"
  },
  "oneTimePreKeys": [
    { "keyId": 0, "publicKey": "base64(OPK_pub)" }
  ],
  "registrationId": 12345
}
```

**Response** `201 Created`

---

### `GET /api/keys/:userId`

Fetch a prekey bundle for initiating an X3DH session. Atomically marks one one-time prekey as used.

**Response** `200 OK`
```json
{
  "registrationId": 12345,
  "identityKey": "base64(IK_pub)",
  "signedPreKey": {
    "keyId": 1,
    "publicKey": "base64(SPK_pub)",
    "signature": "base64(sig)"
  },
  "oneTimePreKey": {
    "keyId": 0,
    "publicKey": "base64(OPK_pub)"
  }
}
```

`oneTimePreKey` is `null` if the pool is exhausted.

---

### `POST /api/keys/:userId/prekeys`

Upload additional one-time prekeys to replenish the pool.

**Request body** — array of `{ keyId, publicKey }` objects.

**Response** `204 No Content`

---

### `POST /api/messages`

Send an encrypted envelope to a recipient. If the recipient has an active WebSocket connection the envelope is pushed immediately; otherwise it is stored for later retrieval.

**Request body**
```json
{
  "recipientId": "string",
  "senderIk": "base64(sender IK_pub)",
  "ephemeralKey": "base64(EK_A)",
  "otpkId": 0,
  "ciphertext": "base64(ratchet-encrypted payload)"
}
```

`ephemeralKey` and `otpkId` are only present on the **first** message to a recipient (X3DH initiator message). Subsequent messages omit them.

**Response** `202 Accepted`

---

### `GET /api/messages/:userId`

Fetch all undelivered messages for a user (offline drain). All returned messages are immediately marked as delivered.

**Response** `200 OK` — array of stored message objects.

---

### `GET /ws/:userId`

WebSocket upgrade endpoint. Connect once per session and keep alive for real-time delivery.

**Server → client push** (JSON)
```json
{
  "type": "message",
  "payload": {
    "id": "uuid",
    "senderId": "string",
    "senderIk": "base64",
    "ephemeralKey": "base64 | null",
    "ciphertext": "base64",
    "timestamp": 1748900000000
  }
}
```

**Client → server** (acknowledgement)
```json
{ "type": "ack", "messageId": "uuid" }
```

---

## Database schema

```sql
-- Registered devices
devices (user_id PK, display_name, identity_key, spk_public, spk_signature, reg_id, registered_at)

-- One-time prekey pool — each key consumed at most once
one_time_prekeys (id AUTOINCREMENT, user_id FK, key_id, public_key, used)

-- Sealed message store — ciphertext is opaque to the server
messages (id PK, recipient_id, sender_id, sender_ik, ephemeral_key, otpk_id, ciphertext, timestamp, delivered)
```

All tables are created on first startup. The database uses WAL journal mode for safe concurrent reads.

---

## Tech stack

| | |
|---|---|
| HTTP + WebSocket | [Axum 0.7](https://github.com/tokio-rs/axum) |
| Async runtime | [Tokio](https://tokio.rs) |
| Database | SQLite via [rusqlite](https://github.com/rusqlite/rusqlite) (statically bundled) |
| Live connections | [DashMap](https://github.com/xacrimon/dashmap) of `mpsc` channels |
| CORS / tracing | tower-http |

---

## Security notes

- **The server is not an authentication authority.** User IDs are currently derived from the client's identity public key. A production deployment should add token-based authentication (e.g. JWT issued after phone/email verification) and validate the sender identity on every request.
- **One-time prekeys** provide forward secrecy for the first message. If the pool empties, sessions fall back to the signed prekey only — clients should replenish via `POST /api/keys/:id/prekeys` when the pool is low.
- **TLS is required** in any non-localhost deployment. Run behind a reverse proxy (nginx, Caddy) that terminates TLS and proxies both HTTP and the `/ws/*` WebSocket upgrade.
