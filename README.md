# engage

End-to-end encrypted desktop chat — built with Tauri 2, Vue 3, and Rust.

Messages are encrypted on your device before leaving it. The relay server forwards sealed envelopes and never has access to plaintext.

---

## Architecture

```
┌──────────────────────────┐        ┌────────────────────────────┐
│   engage (this repo)     │        │   engage-server            │
│                          │        │                            │
│  Vue 3 frontend          │  WSS   │  Axum relay server         │
│  ├─ Pinia stores         │◄──────►│  ├─ Key distribution API   │
│  ├─ Vue Router           │  HTTPS │  ├─ Sealed message relay    │
│  └─ Tauri IPC bridge     │        │  └─ WebSocket push         │
│                          │        │                            │
│  Rust backend (Tauri)    │        │  SQLite (server-side)      │
│  ├─ X3DH key agreement   │        │  (stores only ciphertext)  │
│  ├─ Double Ratchet       │        └────────────────────────────┘
│  └─ SQLite (local)       │
└──────────────────────────┘
```

### Cryptography stack

| Primitive | Role | Crate |
|---|---|---|
| X25519 | Key agreement (X3DH + Double Ratchet DH steps) | `x25519-dalek` |
| Ed25519 | Signed prekey signatures | `ed25519-dalek` |
| AES-256-GCM | Message encryption | `aes-gcm` |
| HKDF-SHA256 | Key derivation (X3DH output + ratchet KDF) | `hkdf` / `sha2` |

The full [X3DH](https://signal.org/docs/specifications/x3dh/) + [Double Ratchet](https://signal.org/docs/specifications/doubleratchet/) protocol is implemented in pure Rust in `src-tauri/src/crypto/`.

---

## Repository layout

```
engage/
├── src/                        # Vue 3 frontend
│   ├── config.ts               # Server URL config (VITE_SERVER_URL)
│   ├── router/index.ts         # Vue Router (hash history)
│   ├── stores/
│   │   ├── identity.ts         # Key generation, registration, WS connect
│   │   ├── contacts.ts         # Contact list + X3DH session setup
│   │   └── messages.ts         # Send (encrypt → relay) / receive (decrypt)
│   ├── composables/
│   │   ├── useWebSocket.ts     # WS singleton with auto-reconnect + decrypt
│   │   ├── useServerApi.ts     # Typed fetch wrapper for the relay server
│   │   └── useCrypto.ts        # Thin wrappers over Tauri crypto commands
│   ├── views/
│   │   ├── SetupView.vue       # First-run identity creation
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
    └── tauri.conf.json
```

---

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Rust | ≥ 1.96 | Install via [rustup](https://rustup.rs) |
| Node.js | ≥ 18 | v19 also works (engine warnings are non-fatal) |
| npm | ≥ 8 | Bundled with Node |
| C linker | — | **Windows:** MinGW GCC via Scoop (`scoop install gcc`) or MSVC Build Tools. **macOS/Linux:** Xcode CLT / `build-essential` |
| engage-server | running | See [engage-server](https://github.com/faridguzman91/rust-engage/tree/engage-server) |

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

### 2. Start the relay server

The client needs the server running before first launch. See the [server README](https://github.com/faridguzman91/rust-engage/tree/engage-server) or run:

```bash
# In a separate terminal — requires Rust toolchain
git clone --branch engage-server git@github.com:faridguzman91/rust-engage.git engage-server
cd engage-server
cargo run
# Server listens on http://localhost:3000
```

### 3. Install frontend dependencies

```bash
npm install
```

### 4. Run in development mode

```bash
npm run tauri dev
```

Tauri will start the Vite dev server on `http://localhost:1420` and open the native app window.

### 5. First run

On first launch you will be taken to the **Setup** screen. Enter a display name — the app will:

1. Generate your Ed25519 identity key pair and X25519 signed prekey locally
2. Register your public keys with the relay server (`POST /api/register`)
3. Open a WebSocket connection for real-time message delivery

---

## Configuration

### Frontend

Create a `.env.local` file in the project root to override the default server URL:

```env
VITE_SERVER_URL=http://localhost:3000
```

The WebSocket URL is derived automatically (`http://` → `ws://`, `https://` → `wss://`).

### Server

The relay server reads these environment variables at startup:

| Variable | Default | Description |
|---|---|---|
| `PORT` | `3000` | TCP port to listen on |
| `DATABASE_PATH` | `engage-server.db` | Path to the SQLite database file |

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
                                  (never decrypts)
                                                            6. receive WS envelope
                                                            7. X3DH receive (EK_A)
                                                            8. init Double Ratchet
                                                            9. decrypt → "hello"
```

After the first message, both sides advance the Double Ratchet independently — each message uses a fresh key derived from the ratchet chain, providing **forward secrecy** and **break-in recovery**.

---

## Building for production

```bash
npm run tauri build
```

Binaries are written to `src-tauri/target/release/bundle/`.

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
| Local storage | SQLite via [rusqlite](https://github.com/rusqlite/rusqlite) (bundled) |
| Relay server | [Axum](https://github.com/tokio-rs/axum) + Tokio |

---

## Roadmap

- [ ] Authentication (token-based — currently identity key is used directly as user ID)
- [ ] One-time prekey replenishment (auto-upload when pool runs low)
- [ ] Group messaging
- [ ] Voice/video via WebRTC + TURN
- [ ] Mobile (Tauri Android/iOS target)
- [ ] Disappearing messages
