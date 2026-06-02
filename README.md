# engage-server

Relay server for the [engage](https://github.com/faridguzman91/rust-engage) encrypted chat app.

The server is a **zero-knowledge forwarder** — it stores and delivers sealed envelopes but never holds decryption keys and cannot read message content. Identity is verified via Google OAuth 2.0; all protected endpoints require a JWT issued after successful login.

---

## Architecture

```
Clients                          engage-server
───────                          ─────────────
GET  /api/auth/google      ──►   Redirect to Google consent screen
GET  /api/auth/google/callback ► Exchange code → issue JWT → deep-link back to app

POST /api/register         ──►   Store public keys (IK, SPK, OPKs)        [auth required]
GET  /api/keys/:id         ──►   Return prekey bundle (claims one OPK)     [auth required]
POST /api/keys/:id/prekeys ──►   Replenish one-time prekeys                [auth required]
POST /api/messages         ──►   Store ciphertext, push to WS if online    [auth required]
GET  /api/messages/:id     ──►   Fetch + mark delivered (offline drain)    [auth required]
GET  /ws/:id?token=JWT     ──►   WebSocket — real-time delivery channel    [auth required]
```

All message payloads are opaque `ciphertext` blobs encrypted by the client. The server never attempts to decrypt them. The `sender_id` on every message is taken from the JWT — the client cannot forge it.

---

## Repository layout

```
engage-server/
├── src/
│   ├── main.rs              # Axum router, middleware wiring, startup
│   ├── auth.rs              # issue_jwt / verify_jwt (HS256) + require_auth middleware
│   ├── db.rs                # SQLite schema + WAL-mode connection
│   ├── models.rs            # Request/response types + WsEnvelope
│   ├── state.rs             # AppState (db + connections + OAuthConfig)
│   └── routes/
│       ├── oauth.rs         # Google OAuth flow, CSRF state, JWT issuance
│       ├── keys.rs          # /api/register, /api/keys/:id, /api/keys/:id/prekeys
│       ├── messages.rs      # /api/messages (send + fetch)
│       └── ws.rs            # /ws/:user_id WebSocket handler (JWT via query param)
├── .env.example             # Environment variable template
└── Cargo.toml
```

---

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Rust | ≥ 1.96 | Install via [rustup](https://rustup.rs) |
| C linker | — | **Windows:** MinGW GCC (`scoop install gcc`) or MSVC Build Tools. **macOS/Linux:** Xcode CLT / `build-essential` |
| Google OAuth credentials | — | See setup below |

### Windows toolchain note

The project targets `x86_64-pc-windows-gnu` (set in `rust-toolchain.toml`):

```powershell
scoop install gcc
rustup toolchain install stable-x86_64-pc-windows-gnu
```

---

## Setup

### 1. Google OAuth credentials

1. Go to [Google Cloud Console](https://console.cloud.google.com/) → **APIs & Services** → **Credentials**
2. Create an **OAuth 2.0 Client ID** — application type: **Web application**
3. Add your callback URL to **Authorized redirect URIs**:
   - Development: `http://localhost:3000/api/auth/google/callback`
   - Production: `https://yourdomain.com/api/auth/google/callback`

### 2. Configure environment

```bash
cp .env.example .env
```

Edit `.env`:

```env
GOOGLE_CLIENT_ID=your-client-id.apps.googleusercontent.com
GOOGLE_CLIENT_SECRET=your-client-secret
GOOGLE_REDIRECT_URI=http://localhost:3000/api/auth/google/callback
JWT_SECRET=<output of: openssl rand -hex 32>
```

### 3. Run

```bash
cargo run
```

Server starts on `http://0.0.0.0:3000` by default.

---

## Environment variables

| Variable | Default | Required | Description |
|---|---|---|---|
| `GOOGLE_CLIENT_ID` | — | **Yes** | OAuth 2.0 client ID from Google Cloud Console |
| `GOOGLE_CLIENT_SECRET` | — | **Yes** | OAuth 2.0 client secret |
| `GOOGLE_REDIRECT_URI` | `http://localhost:3000/api/auth/google/callback` | No | Must match Google Cloud Console setting |
| `JWT_SECRET` | — | **Yes** | Secret for signing HS256 JWTs — use a long random string |
| `PORT` | `3000` | No | TCP port to listen on |
| `DATABASE_PATH` | `engage-server.db` | No | Path to the SQLite database file |
| `RUST_LOG` | _(unset)_ | No | Log filter — e.g. `engage_server=debug,tower_http=info` |

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

All endpoints marked **[protected]** require an `Authorization: Bearer <JWT>` header (or `?token=<JWT>` for WebSocket).

---

### Authentication

#### `GET /api/auth/google`

Redirects the browser to Google's OAuth consent screen. Open this URL in the system browser.

**Response** `302 Found` → Google

---

#### `GET /api/auth/google/callback`

Google redirects here after the user grants consent. The server:
1. Validates the CSRF `state` parameter
2. Exchanges the `code` for a Google ID token
3. Fetches the user's `sub`, `email`, and `name` from Google
4. Creates or looks up the user account
5. Issues a 30-day HS256 JWT
6. Redirects to `engage://auth?token=<JWT>` (caught by the Tauri deep-link handler)

**Response** `302 Found` → `engage://auth?token=…`

---

### Key distribution

#### `POST /api/register` [protected]

Upload or refresh crypto keys for the authenticated user. The `user_id` is always taken from the JWT — the request body cannot override it.

**Request body**
```json
{
  "displayName": "Alice",
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

#### `GET /api/keys/:userId` [protected]

Fetch a prekey bundle to initiate an X3DH session with `:userId`. Atomically marks one one-time prekey as used so it is never served twice.

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

`oneTimePreKey` is `null` if the pool is exhausted (session falls back to SPK only).

---

#### `POST /api/keys/:userId/prekeys` [protected]

Replenish the one-time prekey pool. Only the authenticated user can upload keys for themselves.

**Request body** — array of `{ keyId, publicKey }` objects.

**Response** `204 No Content`

---

### Message relay

#### `POST /api/messages` [protected]

Send an encrypted envelope. The `sender_id` is set from the JWT — the body cannot forge it. If the recipient has an active WebSocket connection, the envelope is pushed immediately; otherwise it is queued.

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

`ephemeralKey` and `otpkId` are only included on the **first** message to a recipient (X3DH initiator envelope). Omit them for all subsequent messages.

**Response** `202 Accepted`

---

#### `GET /api/messages/:userId` [protected]

Fetch all undelivered messages for the authenticated user. Returns immediately and marks all returned messages as delivered. Only the user themselves can fetch their own messages.

**Response** `200 OK` — array of stored message objects.

---

### WebSocket

#### `GET /ws/:userId?token=<JWT>`

WebSocket upgrade. The JWT is validated before the upgrade completes — a mismatched or expired token returns `401` without upgrading. The `:userId` must match the `sub` claim in the token.

**Server → client** (JSON push)
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
-- Registered devices (one row per user)
devices (
  user_id PK,           -- Google sub
  display_name,
  identity_key,         -- base64 X25519 IK_pub
  spk_public,           -- base64 X25519 SPK_pub
  spk_signature,        -- base64 Ed25519 signature of SPK
  reg_id,
  registered_at
)

-- One-time prekey pool — each key served at most once
one_time_prekeys (id AUTOINCREMENT, user_id FK, key_id, public_key, used)

-- Sealed message store — ciphertext is opaque to the server
messages (
  id PK, recipient_id, sender_id,
  sender_ik,            -- for X3DH first-message session setup
  ephemeral_key,        -- X3DH EK_A (first message only)
  otpk_id,
  ciphertext,           -- ratchet-encrypted, never read by server
  timestamp, delivered
)

-- Google OAuth account linking
oauth_accounts (google_sub PK, user_id FK, email)

-- Short-lived CSRF state tokens for the OAuth flow (TTL: 5 min)
oauth_states (state PK, created_at)
```

All tables are created automatically on first startup. WAL journal mode is enabled for safe concurrent access.

---

## Tech stack

| | |
|---|---|
| HTTP + WebSocket | [Axum 0.7](https://github.com/tokio-rs/axum) |
| Async runtime | [Tokio](https://tokio.rs) |
| Auth | Google OAuth 2.0 + HS256 JWT ([jsonwebtoken](https://github.com/Keats/jsonwebtoken)) |
| HTTP client | [reqwest](https://github.com/seanmonstar/reqwest) (Google token exchange) |
| Database | SQLite via [rusqlite](https://github.com/rusqlite/rusqlite) (statically bundled) |
| Live connections | [DashMap](https://github.com/xacrimon/dashmap) of `mpsc` channels |
| CORS / tracing | tower-http |

---

## Security notes

- **Sender identity is server-enforced.** The `sender_id` on every stored message comes from the JWT `sub` claim, not the request body — a compromised client cannot impersonate another user.
- **One-time prekeys** provide forward secrecy for the first message. If the pool empties, sessions fall back to the signed prekey only — clients should replenish via `POST /api/keys/:id/prekeys` proactively.
- **JWTs are 30-day tokens.** The server is stateless — there is no revocation list. For stricter security, reduce `JWT_EXPIRY_SECS` in `src/auth.rs` and implement a refresh token flow.
- **TLS is required** in any non-localhost deployment. Run behind a reverse proxy (nginx, Caddy) that terminates TLS and proxies both HTTP (`/api/*`) and WebSocket (`/ws/*`) traffic.
- **CSRF protection** for the OAuth flow uses a 32-byte random state token stored in the database with a 5-minute TTL.
