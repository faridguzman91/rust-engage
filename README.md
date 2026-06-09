<p align="center">
  <img src="engage.svg" alt="engage" width="480" />
</p>

---

End-to-end encrypted chat — desktop today, Android next. Built with Tauri 2, Vue 3, and Rust.

![engage chat UI](screenshot.png)

Messages are encrypted on your device before leaving it. The relay server forwards sealed envelopes and never has access to plaintext. Identity is verified via Google OAuth; sessions are authenticated with JWTs.

> **Author:** [@faridguzman91](https://github.com/faridguzman91)

---

## Architecture

```
┌─────────────────────────────────────┐        ┌──────────────────────────────────────┐
│   engage (this repo)                │        │   engage-server                      │
│                                     │        │                                      │
│  Vue 3 + PrimeVue UI                │  WSS   │  Axum relay server                   │
│  ├─ Pinia stores                    │◄──────►│  ├─ Google OAuth + JWT               │
│  ├─ Vue Router                      │  HTTPS │  ├─ Key distribution API             │
│  └─ Tauri IPC bridge                │        │  ├─ Sealed message relay             │
│                                     │        │  ├─ Group fan-out                    │
│  Rust backend (Tauri)               │        │  ├─ WebSocket push                   │
│  ├─ X3DH key agreement              │        │  ├─ Sequence counters (gap detect)   │
│  ├─ Double Ratchet                  │        │  └─ Invite token issuance            │
│  ├─ Sender Keys (groups)            │        │                                      │
│  ├─ pending_messages retry queue    │        │  SQLite (server-side)                │
│  └─ SQLite (local)                  │        │  (stores only ciphertext)            │
└─────────────────────────────────────┘        └──────────────────────────────────────┘

Platforms:  Desktop (Windows / macOS / Linux)  ·  Android (Tauri 2 mobile target)
```

### Cryptography stack

| Primitive | Role | Crate |
|---|---|---|
| X25519 | Key agreement (X3DH + Double Ratchet DH steps) | `x25519-dalek` |
| Ed25519 | Signed prekey signatures | `ed25519-dalek` |
| AES-256-GCM | 1:1 + group message encryption | `aes-gcm` |
| HKDF-SHA256 | Key derivation (X3DH, ratchet KDF, Sender Key ratchet) | `hkdf` / `sha2` |
| Sender Keys | Group message encryption — one encrypt, N recipients | `aes-gcm` |
| HS256 JWT | Session authentication | `jsonwebtoken` |

The full [X3DH](https://signal.org/docs/specifications/x3dh/) + [Double Ratchet](https://signal.org/docs/specifications/doubleratchet/) + Sender Keys protocol is implemented in pure Rust in `src-tauri/src/crypto/`.

---

## Features

### 🔐 End-to-end encryption
- **X3DH** key agreement on first message — start a conversation with someone who is offline
- **Double Ratchet** for every 1:1 message — forward secrecy + break-in recovery
- **Sender Keys** for groups — one encryption, N recipients, server sees a single ciphertext
- All crypto runs in the Tauri Rust backend; the frontend only sees plaintext

### 💬 Message reliability (Phase 1–3)
- **Delivery receipts** — server forwards `ack` from recipient back to sender; `✓` → `✓✓`
- **Read receipts** — emitted when recipient opens the conversation thread; `✓✓` (filled)
- **Optimistic send** — message shows immediately as `sending`; updates to `sent` / `failed`
- **Retry queue** — sealed envelopes persisted in `pending_messages` SQLite table; replayed on every WebSocket reconnect; crash-safe (queued before the POST attempt)
- **Sequence numbers** — per-recipient monotonic counter on every envelope; client detects gaps and automatically drains the server queue to recover missed messages

### 📬 Invites
- `POST /api/invites` — 24-hour single-use token (32 random bytes, hex-encoded)
- `GET /api/invites/:token` — public endpoint returns inviter's display name + identity key
- Settings panel: generate link → copy, QR code (brand colours via `qrcode`), share via email or SMS
- `engage://invite?token=TOKEN` deep link → `/invite` acceptance screen → `addContact` → navigate to chat

### 📱 Android
- Tauri 2 Android target — same Rust crypto core and Vue 3 frontend as desktop
- App Links (`https://engage.app/auth`, `https://engage.app/invite`) for OAuth and invites
- CI pipeline builds a debug APK on every push (see `.github/workflows/android.yml`)
- Full setup guide: [ANDROID.md](ANDROID.md)

### 👥 Group messaging
- Create groups, add/remove members
- Sender Keys fan-out — one ciphertext stored per member
- Sender name shown above each received bubble

### ⏱ Disappearing messages
- Per-conversation TTL (5 s → 1 week) configurable from the chat header
- Auto-deleted on both sides after the timer fires; local sweep every 30 seconds

### 📇 Gmail contact import
- "Find from Gmail" discovers which Gmail contacts are already on engage
- Server handles OAuth token refresh automatically; no contact data stored

### 🔑 OPK replenishment
- One-time prekey pool monitored on every WebSocket connect and X3DH session init
- Auto-uploads a batch of 100 fresh OPKs when pool drops below 10

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

| Screen | Route | Notes |
|---|---|---|
| **Login** | `/login` | Google sign-in card |
| **OAuth callback** | `/auth` | Extracts `?token=` from URL, stores JWT, navigates |
| **Setup** | `/setup` | Display name + key generation |
| **Chat (1:1)** | `/chat/:id` | Two-panel shell with `MessageThread` |
| **Chat (group)** | `/group/:id` | `GroupView` — `AvatarGroup` header, sender names |
| **Settings** | `/settings` | Profile, identity keys, invite panel, sign out |
| **Invite** | `/invite` | Accept an invite link — shows inviter card, Add / Decline |

### Components

#### `ConversationList`
- Brand header with `pi-pencil` (new 1:1), `pi-users` (new group), `pi-cog` (settings), Gmail import
- Self-identity chip with `Avatar` + name + "You" tag
- Tabs — "Direct" and "Groups" sections
- New conversation dialog (name + identity key) · New group dialog (name + member picker)

#### `MessageThread` (1:1)
- Header: contact `Avatar`, name, E2E encrypted `Tag`, disappear timer picker
- Signal-style bubbles: timestamp, delivery status icon (`pi-check` / `pi-check-circle`), expiry countdown
- Composer: pill `InputText`, send button; attach + emoji placeholders

#### `GroupView` (groups)
- `AvatarGroup` header with member count
- Sender name above each received bubble
- Same composer as 1:1, encrypts with Sender Keys

#### `SettingsView`
- Profile panel, identity keys panel (collapsible)
- **Invite panel** — generate link, copy button, QR code, email/SMS share
- Sign out

---

## Message flows

### Send pipeline (optimistic + retry)

```
User hits send
  │
  ├─ encrypt_message (Double Ratchet — ratchet advances once)
  ├─ send_message Tauri → persists locally, status = "sending"
  ├─ queue_pending_message → envelope saved to SQLite (crash-safe)
  ├─ POST /api/messages
  │   ✓ success → status = "sent", remove from pending queue
  │   ✗ failure → status = "failed", stays in queue
  │
  └─ On next WS reconnect: drainPending() retries all queued envelopes
```

### Delivery + read receipts

```
Alice sends message (status: sent)
  └─► Bob's client receives → sends { type: "ack", messageId }
        └─► Server looks up sender_id → pushes Ack to Alice's WS
              └─► Alice: status = "delivered"  (pi-check-circle)

Bob opens the thread
  └─► Client emits { type: "read", messageId } for every received message
        └─► Server → pushes Read to Alice's WS
              └─► Alice: status = "read"  (pi-check-circle filled)
```

### Sequence number gap detection

```
Bob expects seq 4, receives seq 5
  └─► gap detected (missed seq 4)
        └─► drainMissed() → GET /api/messages/bob
              └─► server returns undelivered seq 4
                    └─► decrypt → append → lastSeq = 5
```

### 1:1 messages (Double Ratchet)

```
Alice                             Server                    Bob
─────                             ──────                    ───
fetchPreKeyBundle(bob_id) ──────► GET /api/keys/bob ──────► public keys
X3DH key agreement → shared_secret + EK_A
init Double Ratchet
encrypt("hello") via ratchet
POST /api/messages ─────────────► store ciphertext ────────► push via WebSocket
{ ciphertext, EK_A, JWT }         (never decrypts)           X3DH receive (EK_A)
                                                              init Double Ratchet
                                                              decrypt → "hello"
```

### Group messages (Sender Keys)

```
Alice creates group "Team" with Bob, Carol
  └─► distribute SenderKey to Bob (encrypted via pairwise ratchet)
  └─► distribute SenderKey to Carol (encrypted via pairwise ratchet)

Alice sends "Hello team!":
  encrypt("Hello team!") with Alice's SenderKey → one ciphertext
  POST /api/groups/:id/messages
    └─► server stores row for Bob, row for Carol (same ciphertext)
    └─► pushes via WS to Bob and Carol if online

Bob receives:
  decrypt with Alice's stored SenderKey → "Hello team!"
  Alice's SenderKey ratchets forward on Bob's side
```

### Invite flow

```
Alice (Settings) → "Generate invite link"
  └─► POST /api/invites → { token, url: "engage://invite?token=TOKEN" }
        └─► QR code + copy + email/SMS share

Bob taps the link → engage://invite?token=TOKEN
  └─► App.vue deep link handler → router.push("/invite?token=TOKEN")
        └─► GET /api/invites/:token (public, no auth)
              └─► returns { userId, displayName, identityKey }
                    └─► Bob clicks "Add contact"
                          └─► addContact(ik, name) → /chat/:aliceId
```

### Authentication flow

```
Tauri webview                   Server                    Google
─────────────                   ──────                    ──────
window.location.href ─────────► GET /api/auth/google ──► OAuth consent
                                POST token exchange  ──► id_token + access_token
                                Store tokens in DB
                                issue HS256 JWT
Dev:  redirect ◄─────────────── localhost:1420/#/auth?token=JWT
Prod: redirect ◄─────────────── engage://auth?token=JWT
JWT stored in localStorage
All requests: Authorization: Bearer JWT
WS: /ws/:userId?token=JWT
```

---

## Repository layout

```
engage/
├── .cargo/config.toml              # rust-lld linker for Windows GNU target
├── .github/workflows/
│   └── android.yml                 # Android CI — debug APK on every push
├── ANDROID.md                      # Android port developer setup guide
├── Makefile                        # dev / build / android-* / docker-* targets
├── docker-compose.yml              # Compose file for the relay server
│
├── src/                            # ── Server (Axum relay) ──────────────────
│   ├── main.rs                     # Route registration, server entry point
│   ├── auth.rs                     # JWT middleware
│   ├── db.rs                       # Server SQLite schema + migrations
│   ├── models.rs                   # Request/response + WsEnvelope types
│   ├── state.rs                    # AppState (DB, WS connections map)
│   └── routes/
│       ├── oauth.rs                # Google OAuth start + callback
│       ├── keys.rs                 # Prekey bundle, OPK upload/count
│       ├── messages.rs             # Send envelope, fetch offline, seq counter
│       ├── groups.rs               # Group CRUD + fan-out delivery
│       ├── invites.rs              # Create + redeem invite tokens
│       └── ws.rs                   # WebSocket handler — ack/read forwarding
│
├── src/                            # ── Frontend (Vue 3) ─────────────────────
│   ├── config.ts                   # VITE_SERVER_URL
│   ├── main.ts                     # PrimeVue + Pinia + Router setup
│   ├── styles/global.css           # Design tokens, PrimeVue dark overrides
│   ├── router/index.ts             # Auth + identity route guards
│   ├── App.vue                     # Deep link handler (engage://)
│   │
│   ├── stores/
│   │   ├── auth.ts                 # JWT storage, Google OAuth
│   │   ├── identity.ts             # Key generation, server registration, WS connect
│   │   ├── contacts.ts             # Contact CRUD + X3DH session init
│   │   ├── messages.ts             # Send / receive / retry queue / status updates
│   │   └── groups.ts               # Group CRUD, Sender Key distribute/encrypt/decrypt
│   │
│   ├── composables/
│   │   ├── useWebSocket.ts         # WS singleton — receipts, gap detection, drain
│   │   ├── useServerApi.ts         # Typed fetch — all API endpoints incl. invites
│   │   ├── useOpkReplenishment.ts  # OPK pool check → generate → upload
│   │   ├── useDisappearingMessages.ts # TTL timers, sweep, countdown
│   │   └── useCrypto.ts            # Thin Tauri command wrappers
│   │
│   ├── views/
│   │   ├── LoginView.vue           # Google sign-in card
│   │   ├── AuthCallbackView.vue    # OAuth callback — extracts ?token= from URL
│   │   ├── SetupView.vue           # Display name + key generation
│   │   ├── ChatView.vue            # Two-panel shell (1:1)
│   │   ├── GroupView.vue           # Group conversation thread
│   │   ├── SettingsView.vue        # Profile, keys, invite panel, sign out
│   │   └── InviteView.vue          # Invite acceptance — shows inviter, Add / Decline
│   │
│   └── components/
│       ├── ConversationList.vue    # Sidebar — Direct/Groups tabs, new dialogs
│       └── MessageThread.vue      # 1:1 bubbles + delivery status + disappear timer
│
└── src-tauri/                      # ── Tauri Rust backend ───────────────────
    ├── Cargo.toml                  # cdylib + staticlib + rlib (desktop + Android)
    ├── tauri.conf.json             # engage:// desktop + App Links mobile deep links
    └── src/
        ├── lib.rs                  # App setup, SQLite init, command registry
        ├── crypto/
        │   ├── x3dh.rs             # X3DH key agreement (initiator + recipient)
        │   ├── ratchet.rs          # Double Ratchet (encrypt/decrypt, skipped keys)
        │   ├── session.rs          # Session manager — X3DH→Ratchet, persists to SQLite
        │   ├── sender_key.rs       # Sender Keys — group encrypt/decrypt, ratchet
        │   ├── identity.rs         # Identity bundle generation
        │   └── keys.rs             # X25519 / Ed25519 helpers
        ├── commands/
        │   ├── identity.rs         # create_identity, get_identity
        │   ├── contacts.rs         # list/add/remove_contact
        │   ├── messages.rs         # list/send/update_status + pending queue commands
        │   ├── crypto.rs           # init_session, init_inbound_session,
        │   │                       # encrypt/decrypt_message, generate_prekey_bundle
        │   ├── prekeys.rs          # get_opk_status, generate_and_store_opks
        │   ├── disappear.rs        # get/set_disappear_timer, sweep_expired_messages
        │   └── groups.rs           # cache_group, encrypt/decrypt_group_message,
        │                           # get/store_sender_key_distribution
        └── storage/db.rs           # SQLite schema + WAL migrations (pending_messages, etc.)
```

---

## Quick start

### Option A — Makefile (recommended)

```bash
# Clone both repos side-by-side
git clone git@github.com:faridguzman91/rust-engage.git engage
git clone --branch engage-server git@github.com:faridguzman91/rust-engage.git engage-server

# Configure server credentials
cd engage-server && cp .env.example .env
# Edit .env: fill in GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET, JWT_SECRET, FRONTEND_URL

# Start server + client simultaneously
cd ../engage && make dev
```

### Option B — Docker Compose (server) + native client

```bash
cd engage && make docker-up   # server in container
make client                   # Tauri desktop client
```

### Option C — Manual

See step-by-step instructions below.

---

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Rust | ≥ 1.76 | Install via [rustup](https://rustup.rs) |
| Node.js | ≥ 18.12 | Recommend Node 22 LTS via [nvm](https://github.com/nvm-sh/nvm) |
| **pnpm** | **≥ 9** | npm is not used |
| Docker | 24+ | Only for `make docker-up` |
| C linker | — | **macOS:** Xcode CLT · **Windows:** GCC 14 via scoop · **Linux:** `build-essential` |

### macOS toolchain

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh && source "$HOME/.cargo/env"
nvm install 22 && nvm use 22
corepack enable && corepack prepare pnpm@9.15.9 --activate
```

**Gotchas:**
- `._*` resource forks — if cloning to an exFAT volume run `find . -name "._*" -delete` before building
- `engage://` deep link — declared statically in `Info.plist` via `tauri.conf.json`; no runtime registration needed on macOS

### Windows toolchain

This project targets `x86_64-pc-windows-gnu`. Visual Studio Build Tools are **not** required.

```powershell
scoop install mingw nodejs-lts
corepack enable && corepack prepare pnpm@9.15.9 --activate
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup override set stable-x86_64-pc-windows-gnu   # run inside src-tauri/
```

`.cargo/config.toml` applies `-fuse-ld=lld` (rust-lld) automatically for this target, which resolves the Windows PE ordinal limit — `cdylib` is now safe to include alongside `staticlib` and `rlib`.

---

## Manual setup

### 1. Clone

```bash
git clone git@github.com:faridguzman91/rust-engage.git
```

### 2. Set up Google credentials

1. [Google Cloud Console](https://console.cloud.google.com/) → **APIs & Services → Credentials**
2. Create **OAuth 2.0 Client ID** — type: **Web application**
3. Add `http://localhost:3000/api/auth/google/callback` to **Authorized redirect URIs**
4. Copy client ID + secret into `engage-server/.env`

**Required APIs:** Google People API (Gmail import) · Google Identity (OAuth)

**Consent screen scopes:** `openid` · `email` · `profile` · `https://www.googleapis.com/auth/contacts.readonly`

### 3. Configure and start the relay server

```bash
git clone --branch engage-server git@github.com:faridguzman91/rust-engage.git engage-server
cd engage-server && cp .env.example .env   # fill in credentials
cargo run
```

### 4. Start the client

```bash
pnpm install
pnpm tauri dev
```

### 5. First run

```
Launch app → /login → "Continue with Google"
  └─► Google consent → Server issues JWT → /#/auth?token=JWT
        └─► AuthCallbackView stores token → /setup
              └─► Enter display name → keys generated + registered
                    └─► /chat → Ready to message
```

---

## Configuration

### Frontend

```env
# .env.local
VITE_SERVER_URL=http://localhost:3000
```

### Server

| Variable | Description |
|---|---|
| `GOOGLE_CLIENT_ID` | From Google Cloud Console |
| `GOOGLE_CLIENT_SECRET` | From Google Cloud Console |
| `JWT_SECRET` | `openssl rand -hex 32` |
| `FRONTEND_URL` | **Dev only** — `http://localhost:1420` |

Full reference: [engage-server/.env.example](https://github.com/faridguzman91/rust-engage/blob/engage-server/.env.example)

---

## Gmail contact import

The **Find from Gmail** button discovers which of your Gmail contacts are already on engage.

```
Client (JWT) → GET /api/contacts/suggest
  Server: load access_token → refresh if expired → GET People API
  Cross-reference emails against devices table (excludes self + unregistered)
  ← [{ userId, displayName, identityKey, email }]
User clicks "Add" → addContact(ik, name) → X3DH session init on next message
```

---

## Production build

```bash
pnpm tauri build
# Binaries → src-tauri/target/release/bundle/
```

For production: use HTTPS, remove `FRONTEND_URL` (falls back to `engage://` deep link), run the server behind nginx/Caddy.

---

## Android build

See [ANDROID.md](ANDROID.md) for the full setup guide. Quick version:

```bash
# 1. Install Android Studio + NDK 30, set ANDROID_HOME + NDK_HOME
# 2. Add Rust Android targets
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk --locked

# 3. Generate Gradle project (once)
make android-init

# 4. Dev build on connected device / emulator
make android-dev

# 5. Release APK
make android-build
```

CI builds a debug APK on every push — see `.github/workflows/android.yml`.

---

## Tech stack

| Layer | Technology |
|---|---|
| Desktop + Android shell | [Tauri 2](https://tauri.app) |
| Frontend framework | [Vue 3](https://vuejs.org) + TypeScript |
| UI component library | [PrimeVue 4](https://primevue.org) — Aura preset + PrimeIcons |
| State management | [Pinia](https://pinia.vuejs.org) |
| Routing | [Vue Router 4](https://router.vuejs.org) |
| QR codes | [qrcode](https://github.com/soldair/node-qrcode) |
| Package manager | [pnpm](https://pnpm.io) |
| Build tool | [Vite](https://vitejs.dev) |
| Crypto (1:1) | X3DH + Double Ratchet — `x25519-dalek`, `ed25519-dalek`, `aes-gcm`, `hkdf` |
| Crypto (groups) | Sender Keys — AES-256-GCM + HKDF ratchet |
| Auth | Google OAuth 2.0 + HS256 JWT |
| Local storage | SQLite via [rusqlite](https://github.com/rusqlite/rusqlite) (bundled) |
| Relay server | [Axum 0.7](https://github.com/tokio-rs/axum) + Tokio |
| Containerisation | Docker + Docker Compose |

---

## Makefile targets

| Target | What it does |
|---|---|
| `make dev` | Start relay server + Tauri desktop client in parallel |
| `make server` | Start relay server only |
| `make client` | Start Tauri desktop client only |
| `make install` | Install / update frontend dependencies |
| `make build` | Production desktop build |
| `make android-init` | Generate `src-tauri/gen/android/` (run once after SDK setup) |
| `make android-dev` | Live-reload dev build on connected Android device or emulator |
| `make android-build` | Build release APK |
| `make docker-up` | Start server via Docker Compose |
| `make docker-down` | Stop Docker Compose services |
| `make clean` | Remove Rust + frontend build artefacts |

---

## Roadmap

- [x] **E2E encryption** — X3DH + Double Ratchet (forward secrecy, break-in recovery)
- [x] **Authentication** — Google OAuth 2.0 + HS256 JWT; all routes and WebSocket connections protected
- [x] **Relay server** — zero-knowledge Axum server; stores and forwards sealed envelopes only
- [x] **Offline message drain** — messages queued server-side; delivered on reconnect
- [x] **OPK replenishment** — auto-upload 100 fresh OPKs when pool drops below 10
- [x] **PrimeVue UI** — Signal-inspired dark theme, PrimeVue 4 + Aura + PrimeIcons
- [x] **Disappearing messages** — per-conversation TTL; auto-delete on both sides
- [x] **Group messaging** — Sender Keys (Signal-style); one encrypt, server fans out
- [x] **Gmail contact import** — "Find from Gmail" via Google People API

### Message Sending — Reliability & Status

- [x] **Delivery receipts** — server forwards `ack` from recipient; `sent → delivered`
- [x] **Read receipts** — emitted when recipient opens thread; `delivered → read`
- [x] **Retry queue** — `pending_messages` table; drained on every WS reconnect; crash-safe
- [x] **Message ordering** — `seq_counters` per recipient; gap detection triggers offline drain

### Invites

- [x] **Invite links** — `POST /api/invites` issues 24h token; `GET /api/invites/:token` (public) returns inviter bundle; deep link handled in `App.vue`
- [x] **QR code** — rendered in Settings with brand colours
- [x] **Share sheet** — copy, `mailto:`, `sms:` via `tauri-plugin-opener`

### Android Port

- [x] `cdylib` re-enabled — Windows PE limit resolved via `rust-lld`
- [x] App Links configured — `https://engage.app/auth` + `/invite` in `tauri.conf.json`; `android:autoVerify="true"` in generated `AndroidManifest.xml`
- [x] SQLite path — `app_data_dir()` resolves to `/data/data/com.engage.app/files/` on Android
- [x] CI pipeline — `.github/workflows/android.yml` builds debug APK on every push (NDK 30, 4 ABI targets)
- [x] Makefile targets — `android-init`, `android-dev`, `android-build`; rustup PATH fix for Windows Scoop users baked in
- [x] Android Gradle project generated and committed — `src-tauri/gen/android/`; `minSdkVersion=24`, `INTERNET` permission, edge-to-edge, cleartext traffic only in debug
- [x] All TypeScript + config errors fixed — `scheme: ["https"]`, `onOpenUrl`, `openUrl`, missing npm packages installed
- [x] Rust cross-compilation verified — `aarch64-linux-android` compiles successfully
- [x] [`ANDROID.md`](ANDROID.md) — full guide: SDK/NDK setup, fingerprint (3 methods), OAuth client, `assetlinks.json`, `VITE_SERVER_URL` per target, signing, CI, troubleshooting
- [ ] **Enable Windows Developer Mode** — final step to complete APK packaging (`Settings → System → For developers`)
- [ ] Set `VITE_SERVER_URL=http://10.0.2.2:3000` in `.env.local` for emulator / LAN IP for device
- [ ] Register Android OAuth 2.0 Client ID in Google Cloud Console with SHA-1 fingerprint
- [ ] Deploy `assetlinks.json` to `https://engage.app/.well-known/` with SHA-256 fingerprint

### iOS Port

- [ ] Add Rust targets: `aarch64-apple-ios`, `x86_64-apple-ios`, `aarch64-apple-ios-sim`
- [ ] Run `pnpm tauri ios init` → generate Xcode project under `src-tauri/gen/ios/`
- [ ] OAuth: `engage://` URL scheme reuses macOS `Info.plist` config
- [ ] Keychain: replace `localStorage` JWT storage with Tauri `stronghold` or `keychain` plugin
- [ ] Provisioning: Apple Developer account, bundle ID `app.engage.client`
- [ ] CI: `macos-latest` runner + `xcode-select`

### Voice / Video Calls (STUN/TURN NAT Traversal)

- [ ] WS signaling — `CallOffer`, `CallAnswer`, `IceCandidate`, `CallHangup` envelope types
- [ ] `useWebRTC.ts` composable — `RTCPeerConnection`, ICE, offer/answer over WS
- [ ] `CallView.vue` — incoming call dialog, `<video>` elements, mute/hang-up
- [ ] Wire up `pi-phone` / `pi-video` buttons in `MessageThread` header
- [ ] STUN — `stun:stun.l.google.com:19302` for development
- [ ] TURN — deploy `coturn` on VPS (`lt-cred-mechanism`, `realm=engage.app`)
- [ ] Short-lived TURN credentials — `/api/turn-credentials` HMAC-SHA1 tokens (24h TTL)
- [ ] Media E2EE — AES-GCM insertable streams on top of DTLS-SRTP

### Microservices Decomposition

- [ ] `auth-svc` — Google OAuth, JWT issue/verify, token refresh
- [ ] `key-svc` — identity key storage, OPK distribution; migrate to Postgres
- [ ] `relay-svc` — message store-and-forward; back with Redis queue
- [ ] `ws-svc` — WebSocket connections, push delivery, presence; Redis pub/sub
- [ ] `group-svc` — group CRUD, Sender Key distribution, fan-out
- [ ] `turn-svc` — TURN credential issuance
- [ ] API gateway — JWT middleware + service routing (nginx or Axum tower)
- [ ] Migration path — `auth-svc` first, then `key-svc`, then `relay-svc` + `ws-svc` together
