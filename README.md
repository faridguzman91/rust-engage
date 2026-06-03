<p align="center">
  <img src="engage.png" alt="engage" width="480" />
</p>

---

End-to-end encrypted desktop chat — built with Tauri 2, Vue 3, and Rust.

Messages are encrypted on your device before leaving it. The relay server forwards sealed envelopes and never has access to plaintext. Identity is verified via Google OAuth; sessions are authenticated with JWTs.

---

## Architecture

```
┌──────────────────────────┐        ┌────────────────────────────┐
│   engage (this repo)     │        │   engage-server            │
│                          │        │                            │
│  Vue 3 frontend          │  WSS   │  Axum relay server         │
│  ├─ Pinia stores         │◄──────►│  ├─ Google OAuth + JWT     │
│  ├─ Vue Router           │  HTTPS │  ├─ Key distribution API   │
│  └─ Tauri IPC bridge     │        │  ├─ Sealed message relay   │
│                          │        │  └─ WebSocket push         │
│  Rust backend (Tauri)    │        │                            │
│  ├─ X3DH key agreement   │        │  SQLite (server-side)      │
│  ├─ Double Ratchet       │        │  (stores only ciphertext)  │
│  └─ SQLite (local)       │        └────────────────────────────┘
└──────────────────────────┘
```

### Cryptography stack

| Primitive | Role | Crate |
|---|---|---|
| X25519 | Key agreement (X3DH + Double Ratchet DH steps) | `x25519-dalek` |
| Ed25519 | Signed prekey signatures | `ed25519-dalek` |
| AES-256-GCM | Message encryption | `aes-gcm` |
| HKDF-SHA256 | Key derivation (X3DH output + ratchet KDF) | `hkdf` / `sha2` |
| HS256 JWT | Session authentication | `jsonwebtoken` |

The full [X3DH](https://signal.org/docs/specifications/x3dh/) + [Double Ratchet](https://signal.org/docs/specifications/doubleratchet/) protocol is implemented in pure Rust in `src-tauri/src/crypto/`.

---

## Repository layout

```
engage/
├── src/                        # Vue 3 frontend
│   ├── config.ts               # Server URL config (VITE_SERVER_URL)
│   ├── router/index.ts         # Vue Router — auth + identity guards
│   ├── stores/
│   │   ├── auth.ts             # JWT storage, Google OAuth login, deep-link listener
│   │   ├── identity.ts         # Key generation, registration, WS connect
│   │   ├── contacts.ts         # Contact list + X3DH session setup
│   │   └── messages.ts         # Send (encrypt → relay) / receive (decrypt)
│   ├── composables/
│   │   ├── useWebSocket.ts     # WS singleton with JWT auth + auto-reconnect
│   │   ├── useServerApi.ts     # Typed fetch wrapper — attaches Bearer token
│   │   └── useCrypto.ts        # Thin wrappers over Tauri crypto commands
│   ├── views/
│   │   ├── LoginView.vue       # Google sign-in screen
│   │   ├── SetupView.vue       # First-run identity / display name setup
│   │   ├── ChatView.vue        # Main two-panel chat layout
│   │   └── SettingsView.vue    # Identity key display
│   └── components/
│       ├── ConversationList.vue
│       └── MessageThread.vue
│
└── src-tauri/                  # Rust / Tauri backend
    ├── src/
    │   ├── crypto/
    │   │   ├── x3dh.rs         # X3DH key agreement (initiator + recipient)
    │   │   ├── ratchet.rs      # Double Ratchet (encrypt/decrypt, skipped keys)
    │   │   ├── session.rs      # Session manager — X3DH→Ratchet, persists to SQLite
    │   │   ├── identity.rs     # Identity bundle generation
    │   │   └── keys.rs         # X25519 / Ed25519 key pair helpers
    │   ├── commands/
    │   │   ├── identity.rs     # create_identity, get_identity
    │   │   ├── contacts.rs     # list/add/remove_contact
    │   │   ├── messages.rs     # list_messages, send_message
    │   │   └── crypto.rs       # init_session, init_inbound_session,
    │   │                       # encrypt_message, decrypt_message,
    │   │                       # generate_prekey_bundle
    │   └── storage/db.rs       # SQLite schema + migrations (WAL mode)
    └── tauri.conf.json         # deep-link scheme: engage://
```

---

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Rust | ≥ 1.96 | Install via [rustup](https://rustup.rs) |
| Node.js | ≥ 18 | v19 also works (engine warnings are non-fatal) |
| npm | ≥ 8 | Bundled with Node |
| C linker | — | **Windows:** MinGW GCC via Scoop (`scoop install gcc`) or MSVC Build Tools. **macOS/Linux:** Xcode CLT / `build-essential` |
| engage-server | running | See [engage-server](https://github.com/faridguzman91/rust-engage/tree/engage-server) — requires Google OAuth credentials |

### Windows-specific toolchain note

This project targets `x86_64-pc-windows-gnu` (set in `src-tauri/rust-toolchain.toml`) to avoid a dependency on the full Visual Studio Build Tools. MinGW's GCC acts as the linker.

```powershell
# Install GCC via Scoop if not already present
scoop install gcc

# Install the GNU Rust toolchain
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup override set stable-x86_64-pc-windows-gnu  # run inside src-tauri/
```

---

## Getting started

### 1. Clone

```bash
git clone git@github.com:faridguzman91/rust-engage.git
cd rust-engage
```

### 2. Set up Google OAuth credentials

1. Go to [Google Cloud Console](https://console.cloud.google.com/) → **APIs & Services** → **Credentials**
2. Create an **OAuth 2.0 Client ID** — application type: **Web application**
3. Add `http://localhost:3000/api/auth/google/callback` to **Authorized redirect URIs**
4. Copy the client ID and secret into the server's `.env` file (see step 3)

### 3. Configure and start the relay server

```bash
# In a separate terminal
git clone --branch engage-server git@github.com:faridguzman91/rust-engage.git engage-server
cd engage-server
cp .env.example .env
# Fill in GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET, and JWT_SECRET in .env
cargo run
# Server listens on http://localhost:3000
```

### 4. Install frontend dependencies

```bash
npm install
```

### 5. Run in development mode

```bash
npm run tauri dev
```

Tauri starts the Vite dev server on `http://localhost:1420` and opens the native app window.

### 6. First run — user flow

```
Launch app
  └─► /login  →  "Continue with Google"
        └─► System browser opens → Google consent
              └─► Server issues JWT → redirects to engage://auth?token=…
                    └─► Tauri catches deep-link → token stored
                          └─► /setup  →  Enter display name → keys generated + registered
                                └─► /chat  →  Ready to message
```

---

## Configuration

### Frontend

Create a `.env.local` file in the project root to override the default server URL:

```env
VITE_SERVER_URL=http://localhost:3000
```

The WebSocket URL is derived automatically (`http://` → `ws://`, `https://` → `wss://`).

### Server

See [engage-server/.env.example](https://github.com/faridguzman91/rust-engage/blob/engage-server/.env.example) for all variables. Required ones:

| Variable | Description |
|---|---|
| `GOOGLE_CLIENT_ID` | From Google Cloud Console |
| `GOOGLE_CLIENT_SECRET` | From Google Cloud Console |
| `JWT_SECRET` | Long random string — `openssl rand -hex 32` |

---

## Authentication flow

```
Client (Tauri)                  Server                    Google
──────────────                  ──────                    ──────
1. open browser ──────────────► GET /api/auth/google ──► OAuth consent
                                                     ◄── auth code
                                POST token exchange  ──► Google
                                                     ◄── id_token + email
                                issue JWT (HS256)
                                redirect ◄────────────── engage://auth?token=JWT
2. deep-link caught
3. JWT stored in localStorage
4. All API calls include:
   Authorization: Bearer JWT
5. WS connects with:
   /ws/:userId?token=JWT
```

---

## Message flow

```
Alice (sender)                    Server                    Bob (receiver)
──────────────                    ──────                    ─────────────
1. fetchPreKeyBundle(bob_id) ──► GET /api/keys/bob ──────► (bob's public keys)
2. X3DH key agreement
   → shared_secret
   → ephemeral_key (EK_A)
3. init Double Ratchet
4. encrypt("hello")
5. POST /api/messages ──────────► store ciphertext ──────► push via WebSocket
   { ciphertext, EK_A, JWT }      (never decrypts)
                                                            6. receive WS envelope
                                                            7. X3DH receive (EK_A)
                                                            8. init Double Ratchet
                                                            9. decrypt → "hello"
```

After the first message, both sides advance the Double Ratchet independently — each message uses a fresh key, providing **forward secrecy** and **break-in recovery**.

---

## Building for production

```bash
npm run tauri build
```

Binaries are written to `src-tauri/target/release/bundle/`.

> For production deployments, point `VITE_SERVER_URL` at your hosted server over HTTPS. The server must run behind a TLS-terminating reverse proxy (nginx, Caddy) so that both the API and WebSocket connections are encrypted in transit.

---

## Tech stack

| Layer | Technology |
|---|---|
| Desktop shell | [Tauri 2](https://tauri.app) |
| Frontend framework | [Vue 3](https://vuejs.org) + TypeScript |
| State management | [Pinia](https://pinia.vuejs.org) |
| Routing | [Vue Router 4](https://router.vuejs.org) |
| Build tool | [Vite](https://vitejs.dev) |
| Crypto (client) | x25519-dalek, ed25519-dalek, aes-gcm, hkdf |
| Auth | Google OAuth 2.0 + HS256 JWT |
| Local storage | SQLite via [rusqlite](https://github.com/rusqlite/rusqlite) (bundled) |
| Relay server | [Axum](https://github.com/tokio-rs/axum) + Tokio |

---

## Roadmap

- [x] **E2E encryption** — X3DH key agreement + Double Ratchet (forward secrecy, break-in recovery)
- [x] **Authentication** — Google OAuth 2.0 + HS256 JWT; all API routes and WebSocket connections are protected
- [x] **Relay server** — zero-knowledge Axum server; stores and forwards sealed envelopes only
- [x] **Offline message drain** — messages queued server-side while recipient is offline, delivered on reconnect
- [x] **OPK replenishment** — auto-upload fresh one-time prekeys when the server pool runs low (watermark: 10, batch: 100)
- [ ] **Disappearing messages** — per-conversation TTL; messages auto-delete on both sides after a set time
- [ ] **Group messaging** — multi-party encrypted chat using Sender Keys (Signal-style)
- [ ] **Voice / video** — WebRTC peer connections + TURN server for NAT traversal
- [ ] **Mobile** — Tauri Android / iOS build target
