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
│  Vue 3 + PrimeVue UI     │  WSS   │  Axum relay server         │
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

## Frontend — PrimeVue UI

The entire interface is built with **[PrimeVue 4](https://primevue.org)** on the **Aura** design preset, themed with a Signal-inspired dark palette.

### Design system

| Token | Value | Usage |
|---|---|---|
| Accent / sent bubbles | `#3ebf8c` | Signal green — brand, sent messages, buttons |
| Received bubbles | `#2a2a3c` | Deep navy |
| Sidebar | `#1e1e2e` | Contact list background |
| Main surface | `#12121c` | Chat area background |
| Header / composer | `#1a1a2a` | Top bar and message input tray |

Dark mode is applied globally via PrimeVue's `darkModeSelector: ".dark"` — the `.dark` class is added to `<html>` on app mount.

### Screens

| Screen | Route | PrimeVue components |
|---|---|---|
| **Login** | `/login` | `Card`, `Button` (Google icon slot), `Message` |
| **Setup** | `/setup` | `Card`, `FloatLabel`, `InputText`, `Button`, `Message` |
| **Chat** | `/chat/:id` | Layout shell — sidebar + thread pane |
| **Settings** | `/settings` | `Panel` (collapsible keys), `Avatar`, `Tag`, `Button`, `Divider` |

### Components

#### `ConversationList`
- Brand header with `pi-pencil` (new conversation) and `pi-cog` (settings) icon buttons
- Self-identity chip with `Avatar` + name + "You" tag
- Contact rows with initial-letter `Avatar`, active highlight, hover state
- **"New conversation"** opens a PrimeVue `Dialog` with `FloatLabel` inputs for name and identity key
- Empty state with `pi-user-plus` prompt

#### `MessageThread`
- Header: contact `Avatar`, name, E2E encrypted `Tag` (green), voice/video call buttons (disabled, roadmap)
- Signal-style message bubbles — green right-aligned (sent), navy left-aligned (received)
- Received messages show the contact's `Avatar` to the left
- Each bubble shows timestamp + `pi-check` / `pi-check-circle` delivery indicator
- `ProgressSpinner` while loading conversation history
- Empty thread state with lock icon prompt
- Composer bar: attach `pi-paperclip` (disabled), `InputText` with rounded pill style, emoji `pi-face-smile` (disabled), send `Button` with accent background

### Icons
All icons use **[PrimeIcons](https://primevue.org/icons/)** (`primeicons` npm package). Key icons used:

`pi-pencil` · `pi-cog` · `pi-lock` · `pi-send` · `pi-check` · `pi-check-circle` · `pi-phone` · `pi-video` · `pi-paperclip` · `pi-face-smile` · `pi-comments` · `pi-user-plus` · `pi-key` · `pi-sign-out` · `pi-arrow-left` · `pi-ellipsis-v`

### Customising the theme

PrimeVue design tokens are overridden in `src/styles/global.css` under the `.dark` selector. To change the accent colour:

```css
.dark {
  --p-primary-color:         #your-color;
  --p-primary-hover-color:   #your-hover;
  --p-primary-active-color:  #your-active;
  --engage-accent:           #your-color;
  --engage-sent-bg:          #your-color;
}
```

---

## Repository layout

```
engage/
├── src/
│   ├── config.ts                   # Server URL (VITE_SERVER_URL env var)
│   ├── main.ts                     # PrimeVue + Pinia + Router setup
│   ├── styles/global.css           # Design tokens, PrimeVue dark overrides
│   ├── router/index.ts             # Auth + identity route guards
│   │
│   ├── stores/
│   │   ├── auth.ts                 # JWT, Google OAuth, deep-link handler
│   │   ├── identity.ts             # Key generation, server registration, WS connect
│   │   ├── contacts.ts             # Contact CRUD + X3DH session init
│   │   └── messages.ts             # Send (encrypt → relay) / receive (decrypt)
│   │
│   ├── composables/
│   │   ├── useWebSocket.ts         # WS singleton — JWT auth, auto-reconnect, OPK trigger
│   │   ├── useServerApi.ts         # Typed fetch — auto Bearer token, 401 redirect
│   │   ├── useOpkReplenishment.ts  # OPK pool check → generate → upload
│   │   └── useCrypto.ts            # Thin Tauri command wrappers
│   │
│   ├── views/
│   │   ├── LoginView.vue           # Google sign-in card
│   │   ├── SetupView.vue           # Display name + key generation
│   │   ├── ChatView.vue            # Two-panel shell
│   │   └── SettingsView.vue        # Profile, keys, sign out
│   │
│   └── components/
│       ├── ConversationList.vue    # Sidebar — contacts, new-conversation dialog
│       └── MessageThread.vue      # Message bubbles + composer
│
└── src-tauri/
    ├── src/
    │   ├── crypto/
    │   │   ├── x3dh.rs             # X3DH key agreement
    │   │   ├── ratchet.rs          # Double Ratchet (encrypt/decrypt, skipped keys)
    │   │   ├── session.rs          # Session manager — persists to SQLite
    │   │   ├── identity.rs         # Identity bundle generation
    │   │   └── keys.rs             # X25519 / Ed25519 helpers
    │   ├── commands/
    │   │   ├── identity.rs         # create_identity, get_identity
    │   │   ├── contacts.rs         # list/add/remove_contact
    │   │   ├── messages.rs         # list_messages, send_message
    │   │   ├── crypto.rs           # init_session, init_inbound_session,
    │   │   │                       # encrypt/decrypt_message, generate_prekey_bundle
    │   │   └── prekeys.rs          # get_opk_status, generate_and_store_opks
    │   └── storage/db.rs           # SQLite schema + WAL migrations
    └── tauri.conf.json             # deep-link scheme: engage://
```

---

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Rust | ≥ 1.96 | Install via [rustup](https://rustup.rs) |
| Node.js | ≥ 18 | v19 also works (engine warnings are non-fatal) |
| **pnpm** | **≥ 7** | **`scoop install pnpm` or `npm i -g pnpm`** — npm is not used |
| C linker | — | **Windows:** see toolchain note below. **macOS/Linux:** Xcode CLT / `build-essential` |
| engage-server | running | See [engage-server](https://github.com/faridguzman91/rust-engage/tree/engage-server) — requires Google OAuth credentials |

### Windows-specific toolchain note

This project targets `x86_64-pc-windows-gnu`. Full Visual Studio Build Tools are **not** required. Instead:

1. **GCC 14** (linker driver) + **LLD** (linker via `rust-lld`) are used.
2. `rust-lld` is bundled with the Rust toolchain — no separate install.
3. GCC 14 is needed to provide `libgcc`, `libmingwex`, etc.

```powershell
# Install GCC 14 via Scoop
scoop install mingw        # GCC 14.2.0

# Install the GNU Rust toolchain
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup override set stable-x86_64-pc-windows-gnu   # run inside src-tauri/
```

The `.cargo/config.toml` at the repo root sets the linker to GCC 14 with `-fuse-ld=lld` automatically — no manual config needed after the above steps.

> **Why not MSVC?** The MSVC Build Tools installer requires ~8 GB. GNU + LLD is a lighter alternative that works on Windows without a full Visual Studio installation.

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
git clone --branch engage-server git@github.com:faridguzman91/rust-engage.git engage-server
cd engage-server
cp .env.example .env
# Fill in GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET, and JWT_SECRET
cargo run
# Listens on http://localhost:3000
```

### 4. Install frontend dependencies

```bash
pnpm install
```

### 5. Run in development mode

```bash
pnpm tauri dev
```

Tauri starts the Vite dev server on `http://localhost:1420` and opens the native window.

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

```env
# .env.local
VITE_SERVER_URL=http://localhost:3000
```

The WebSocket URL is derived automatically (`http://` → `ws://`, `https://` → `wss://`).

### Server

| Variable | Description |
|---|---|
| `GOOGLE_CLIENT_ID` | From Google Cloud Console |
| `GOOGLE_CLIENT_SECRET` | From Google Cloud Console |
| `JWT_SECRET` | Long random string — `openssl rand -hex 32` |

Full reference: [engage-server/.env.example](https://github.com/faridguzman91/rust-engage/blob/engage-server/.env.example)

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
   → shared_secret + EK_A
3. init Double Ratchet
4. encrypt("hello")
5. POST /api/messages ──────────► store ciphertext ──────► push via WebSocket
   { ciphertext, EK_A, JWT }      (never decrypts)
                                                            6. receive WS envelope
                                                            7. X3DH receive (EK_A)
                                                            8. init Double Ratchet
                                                            9. decrypt → "hello"
```

After the first message both sides advance the Double Ratchet independently — each message uses a fresh key, providing **forward secrecy** and **break-in recovery**.

---

## Building for production

```bash
pnpm tauri build
```

Binaries are written to `src-tauri/target/release/bundle/`.

> For production, point `VITE_SERVER_URL` at your server over HTTPS and run the server behind a TLS-terminating proxy (nginx, Caddy) so both HTTP and WebSocket traffic is encrypted in transit.

---

## Tech stack

| Layer | Technology |
|---|---|
| Desktop shell | [Tauri 2](https://tauri.app) |
| Frontend framework | [Vue 3](https://vuejs.org) + TypeScript |
| **UI component library** | **[PrimeVue 4](https://primevue.org) — Aura preset + PrimeIcons** |
| State management | [Pinia](https://pinia.vuejs.org) |
| Routing | [Vue Router 4](https://router.vuejs.org) |
| Build tool | [Vite](https://vitejs.dev) |
| Crypto (client) | x25519-dalek, ed25519-dalek, aes-gcm, hkdf |
| Auth | Google OAuth 2.0 + HS256 JWT |
| Local storage | SQLite via [rusqlite](https://github.com/rusqlite/rusqlite) (bundled) |
| Relay server | [Axum 0.7](https://github.com/tokio-rs/axum) + Tokio |

---

## Roadmap

- [x] **E2E encryption** — X3DH key agreement + Double Ratchet (forward secrecy, break-in recovery)
- [x] **Authentication** — Google OAuth 2.0 + HS256 JWT; all API routes and WebSocket connections are protected
- [x] **Relay server** — zero-knowledge Axum server; stores and forwards sealed envelopes only
- [x] **Offline message drain** — messages queued server-side while recipient is offline, delivered on reconnect
- [x] **OPK replenishment** — auto-upload fresh one-time prekeys when pool drops below 10 (batch of 100)
- [x] **PrimeVue UI** — Signal-inspired dark theme built with PrimeVue 4 + Aura preset + PrimeIcons
- [ ] **Disappearing messages** — per-conversation TTL; messages auto-delete on both sides after a set time
- [ ] **Group messaging** — multi-party encrypted chat using Sender Keys (Signal-style)
- [ ] **Voice / video** — WebRTC peer connections + TURN server for NAT traversal
- [ ] **Mobile** — Tauri Android / iOS build target
