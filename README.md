<p align="center">
  <img src="engage.svg" alt="engage" width="480" />
</p>

---

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-AGPL%20v3-blue.svg" alt="License: AGPL v3" /></a>
  <img src="https://img.shields.io/badge/Rust-1.76+-f74c00?logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/Vue-3-42b883?logo=vue.js&logoColor=white" alt="Vue 3" />
  <img src="https://img.shields.io/badge/TypeScript-5-3178c6?logo=typescript&logoColor=white" alt="TypeScript" />
  <img src="https://img.shields.io/badge/Android-Tauri%202-3ddc84?logo=android&logoColor=white" alt="Android" />
</p>

End-to-end encrypted chat — desktop (Windows / macOS / Linux) and Android. Built with Tauri 2, Vue 3, and Rust.

![engage chat UI](screenshot.png)

Messages are encrypted on your device before leaving it. The relay server forwards sealed envelopes and never has access to plaintext. Identity is verified via Google OAuth; sessions are authenticated with JWTs.

> **Author:** [@faridguzman91](https://github.com/faridguzman91)

---

## Releases

| Version | Highlights |
|---|---|
| [v0.2.0](https://github.com/faridguzman91/rust-engage/releases/tag/v0.2.0) | Message reliability (receipts, retry queue, gap detection), invite system, Android port |
| [v0.1.0](https://github.com/faridguzman91/rust-engage/releases/tag/v0.1.0) | E2EE foundation, groups, disappearing messages, Gmail import |

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
│  Rust backend (Tauri)               │        │  ├─ Sequence counters (gap detect)   │
│  ├─ X3DH key agreement              │        │  ├─ Invite token issuance            │
│  ├─ Double Ratchet                  │        │  └─ WebSocket push + ack forwarding  │
│  ├─ Sender Keys (groups)            │        │                                      │
│  ├─ pending_messages retry queue    │        │  SQLite — ciphertext only            │
│  └─ SQLite (local)                  │        └──────────────────────────────────────┘
└─────────────────────────────────────┘

Platforms:  Desktop (Windows / macOS / Linux)  ·  Android (Tauri 2 — code complete)
```

### Cryptography stack

| Primitive | Role | Crate |
|---|---|---|
| X25519 | Key agreement (X3DH + Double Ratchet DH steps) | `x25519-dalek` |
| Ed25519 | Signed prekey signatures | `ed25519-dalek` |
| AES-256-GCM | 1:1 + group message encryption | `aes-gcm` |
| HKDF-SHA256 | Key derivation (X3DH, ratchet KDF, Sender Key ratchet) | `hkdf` / `sha2` |
| Sender Keys | Group encryption — one encrypt, N recipients | `aes-gcm` |
| HS256 JWT | Session authentication | `jsonwebtoken` |

The full [X3DH](https://signal.org/docs/specifications/x3dh/) + [Double Ratchet](https://signal.org/docs/specifications/doubleratchet/) + Sender Keys protocol is implemented in pure Rust in `src-tauri/src/crypto/`.

---

## Features

### 🔐 End-to-end encryption
- **X3DH** key agreement on first message — start a conversation with someone offline
- **Double Ratchet** for every 1:1 message — forward secrecy + break-in recovery
- **Sender Keys** for groups — one encryption, N recipients, server sees a single ciphertext
- All crypto runs in the Tauri Rust backend; the frontend only sees plaintext

### 💬 Message reliability
- **Delivery receipts** — server forwards `ack` from recipient back to sender; `sent → delivered`
- **Read receipts** — emitted when recipient opens the conversation; `delivered → read`
- **Optimistic send** — message appears immediately as `sending`; updates to `sent` / `failed`
- **Retry queue** — sealed envelope persisted to `pending_messages` SQLite table before every POST; crash-safe; replayed automatically on WS reconnect
- **Sequence numbers** — per-recipient monotonic counter stamped on every envelope; gap detected → server queue drained instantly to recover missed messages

### 📬 Invites
- `POST /api/invites` — 24-hour single-use token (32 random bytes, hex-encoded)
- `GET /api/invites/:token` — public endpoint; returns inviter's display name + identity key; marks token used atomically
- Settings panel: generate link, copy button, QR code (brand colours via `qrcode`), share via email or SMS
- `engage://invite?token=TOKEN` deep link → `/invite` acceptance screen → `addContact` → navigate to chat

### 📱 Android
- **Code complete** — same Rust crypto core and Vue 3 frontend as desktop, compiled for Android via Tauri 2 mobile target
- Android Gradle project in `src-tauri/gen/android/` — `minSdkVersion 24` (Android 7+, ~96% of devices)
- App Links (`https://engage.app/auth`, `https://engage.app/invite`) handle OAuth callbacks and invite deep links
- CI builds a debug APK on every push to `master` — see `.github/workflows/android.yml`
- Full developer setup guide: [ANDROID.md](ANDROID.md)

### 👥 Group messaging
- Create groups, add / remove members
- Sender Keys fan-out — one ciphertext per message, stored once per member on server
- Sender name shown above each received bubble

### ⏱ Disappearing messages
- Per-conversation TTL (5 s → 1 week) configurable from the chat header
- Auto-deleted on both sides after the timer fires; local sweep every 30 s

### 📇 Gmail contact import
- "Find from Gmail" discovers which Gmail contacts are already on engage via Google People API
- Server auto-refreshes OAuth tokens; no contact data stored

### 🔑 OPK replenishment
- One-time prekey pool monitored on every WS connect and X3DH session init
- Auto-uploads 100 fresh OPKs when pool drops below 10

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

Dark mode via PrimeVue's `darkModeSelector: ".dark"` — `.dark` class added to `<html>` on mount.

### Screens

| Screen | Route | Notes |
|---|---|---|
| **Login** | `/login` | Google sign-in card |
| **OAuth callback** | `/auth` | Extracts `?token=` from URL, stores JWT, navigates |
| **Setup** | `/setup` | Display name + key generation |
| **Chat (1:1)** | `/chat/:id` | Two-panel shell with `MessageThread` |
| **Chat (group)** | `/group/:id` | `GroupView` — `AvatarGroup` header, sender names |
| **Settings** | `/settings` | Profile, identity keys, invite panel, sign out |
| **Invite** | `/invite` | Accept invite — inviter card, Add / Decline |

### Components

**`ConversationList`** — brand header, Gmail import, self-identity chip, Direct/Groups tabs, new conversation + new group dialogs

**`MessageThread`** — disappear timer picker, Signal-style bubbles with timestamp + delivery icon (`pi-check` / `pi-check-circle`) + expiry countdown, composer

**`GroupView`** — `AvatarGroup` header, sender names above bubbles, Sender Key composer

**`SettingsView`** — profile, collapsible identity keys, invite panel (generate, copy, QR, share), sign out

---

## Message flows

### Send pipeline

```
User hits send
  ├─ encrypt_message  (Double Ratchet — advances ratchet once, never again for this message)
  ├─ send_message     (Tauri) → persist locally, status = "sending", shown in UI immediately
  ├─ queue_pending    (Tauri) → sealed envelope saved to SQLite before POST (crash-safe)
  ├─ POST /api/messages
  │     ✓ success → status = "sent", dequeue
  │     ✗ failure → status = "failed", stays in queue
  └─ WS reconnect → drainPending() → retry each queued envelope oldest-first
```

### Delivery + read receipts

```
Alice sends → status: sent
  └─► Bob receives → sends { type: "ack", messageId }
        └─► Server: lookup sender_id → push Ack to Alice's WS
              └─► Alice: "delivered"   pi-check-circle

Bob opens thread
  └─► Client emits { type: "read", messageId } for each received message
        └─► Server → push Read to Alice's WS
              └─► Alice: "read"   pi-check-circle (accent colour)
```

### Gap detection

```
Bob expects seq 4, receives seq 5
  └─► checkSeq() detects gap → drainMissed() → GET /api/messages/bob
        └─► server returns undelivered seq 4 → decrypt → append → lastSeq = 5
```

### 1:1 messages (Double Ratchet)

```
Alice                             Server                    Bob
─────                             ──────                    ───
fetchPreKeyBundle(bob_id) ──────► GET /api/keys/bob         public keys
X3DH key agreement → shared_secret + EK_A
init Double Ratchet
encrypt("hello") via ratchet
POST /api/messages ─────────────► store ciphertext ────────► push via WebSocket
{ ciphertext, EK_A, seqNum }      (never decrypts)           X3DH receive → init ratchet
                                                              decrypt → "hello"
                                                              send ack → Alice: delivered
```

### Group messages (Sender Keys)

```
Alice creates group "Team" with Bob, Carol
  └─► distribute SenderKey to Bob   (encrypted via pairwise ratchet)
  └─► distribute SenderKey to Carol (encrypted via pairwise ratchet)

Alice sends "Hello team!":
  encrypt with SenderKey → one ciphertext
  POST /api/groups/:id/messages
    └─► server stores one row per member (same ciphertext, per-member seqNum)
    └─► pushes via WS to online members

Bob receives → decrypt with Alice's SenderKey → "Hello team!"
```

### Invite flow

```
Alice → Settings → "Generate invite link"
  └─► POST /api/invites → { token, url: "engage://invite?token=TOKEN" }
        └─► QR + copy + email/SMS share

Bob taps link → engage://invite?token=TOKEN
  └─► App.vue onOpenUrl() → router.push("/invite?token=TOKEN")
        └─► GET /api/invites/:token (public)
              └─► returns { displayName, identityKey }; marks token used
                    └─► Bob clicks "Add contact" → addContact() → /chat/:aliceId
```

### Authentication flow

```
Tauri webview                   Server                    Google
─────────────                   ──────                    ──────
window.location.href ─────────► GET /api/auth/google ──► OAuth consent
                                POST token exchange  ──► id_token + access_token
                                issue HS256 JWT
Dev:  redirect ◄─────────────── localhost:1420/#/auth?token=JWT
Prod: redirect ◄─────────────── engage://auth?token=JWT
JWT stored in localStorage  ·  Bearer on every request  ·  WS: /ws/:id?token=JWT
```

---

## Repository layout

```
engage/
├── .cargo/config.toml              # rust-lld linker for Windows GNU target
├── .github/workflows/
│   └── android.yml                 # Android CI — debug APK on every push
├── ANDROID.md                      # Android port complete developer guide
├── Makefile                        # dev / build / android-* / docker-* targets
├── docker-compose.yml              # Compose for relay server
│
├── src/                            # ── Axum relay server ────────────────────
│   ├── main.rs                     # Route registration + server entry point
│   ├── auth.rs                     # JWT middleware (require_auth extractor)
│   ├── db.rs                       # SQLite schema + migrations (seq_counters, invite_tokens)
│   ├── models.rs                   # Request/response + WsEnvelope types (Ack, Read, seqNum)
│   ├── state.rs                    # AppState — Mutex<Connection> + DashMap WS connections
│   └── routes/
│       ├── oauth.rs                # Google OAuth start + callback
│       ├── keys.rs                 # Prekey bundle, OPK upload/count
│       ├── messages.rs             # Send envelope, offline fetch, next_seq()
│       ├── groups.rs               # Group CRUD + Sender Key fan-out (per-member seqNum)
│       ├── invites.rs              # POST /api/invites · GET /api/invites/:token
│       └── ws.rs                   # WS upgrade, ack/read frame routing
│
├── src/                            # ── Vue 3 frontend ───────────────────────
│   ├── config.ts                   # VITE_SERVER_URL → SERVER_BASE + SERVER_WS
│   ├── main.ts                     # PrimeVue + Pinia + Router setup
│   ├── styles/global.css           # Design tokens, PrimeVue dark overrides
│   ├── router/index.ts             # Auth + identity route guards; /invite route
│   ├── App.vue                     # onOpenUrl deep link handler (engage://)
│   │
│   ├── stores/
│   │   ├── auth.ts                 # JWT storage, Google OAuth
│   │   ├── identity.ts             # Key generation, server registration, WS connect
│   │   ├── contacts.ts             # Contact CRUD + X3DH session init
│   │   ├── messages.ts             # Optimistic send, retry queue, drainPending, updateStatus
│   │   └── groups.ts               # Group CRUD, Sender Key distribute/encrypt/decrypt
│   │
│   ├── composables/
│   │   ├── useWebSocket.ts         # WS singleton, ack/read receipts, gap detection, drain
│   │   ├── useServerApi.ts         # Typed fetch — all endpoints incl. createInvite/redeemInvite
│   │   ├── useOpkReplenishment.ts  # OPK pool check → generate → upload
│   │   ├── useDisappearingMessages.ts # TTL timers, sweep, countdown
│   │   └── useCrypto.ts            # Tauri command wrappers
│   │
│   ├── views/
│   │   ├── LoginView.vue           # Google sign-in
│   │   ├── AuthCallbackView.vue    # OAuth callback — extracts ?token=, navigates
│   │   ├── SetupView.vue           # Display name + key generation
│   │   ├── ChatView.vue            # Two-panel 1:1 shell
│   │   ├── GroupView.vue           # Group thread (Sender Key composer)
│   │   ├── SettingsView.vue        # Profile · keys · invite panel · sign out
│   │   └── InviteView.vue          # Invite acceptance — inviter card, Add / Decline
│   │
│   └── components/
│       ├── ConversationList.vue    # Sidebar — tabs, Gmail import, new dialogs
│       └── MessageThread.vue      # Bubbles + delivery status + disappear timer
│
└── src-tauri/                      # ── Tauri Rust backend ───────────────────
    ├── Cargo.toml                  # cdylib + staticlib + rlib (desktop + Android/iOS)
    ├── tauri.conf.json             # engage:// (desktop) + App Links (Android/iOS)
    ├── gen/android/                # Generated Android Studio / Gradle project
    └── src/
        ├── lib.rs                  # App init, SQLite open, command registry, deep-link setup
        ├── crypto/
        │   ├── x3dh.rs             # X3DH (initiator + recipient)
        │   ├── ratchet.rs          # Double Ratchet (encrypt/decrypt, skipped-key cache)
        │   ├── session.rs          # Session manager — X3DH→Ratchet, persisted to SQLite
        │   ├── sender_key.rs       # Sender Keys — group encrypt/decrypt + ratchet
        │   ├── identity.rs         # Identity bundle generation
        │   └── keys.rs             # X25519 / Ed25519 key helpers
        ├── commands/
        │   ├── identity.rs         # create_identity, get_identity
        │   ├── contacts.rs         # list/add/remove_contact
        │   ├── messages.rs         # list/send/update_message_status
        │   │                       # queue/list/remove/increment_pending_message
        │   ├── crypto.rs           # init_session, init_inbound_session,
        │   │                       # encrypt/decrypt_message, generate_prekey_bundle
        │   ├── prekeys.rs          # get_opk_status, generate_and_store_opks
        │   ├── disappear.rs        # get/set_disappear_timer, sweep_expired_messages
        │   └── groups.rs           # cache_group, encrypt/decrypt_group_message,
        │                           # get/store_sender_key_distribution
        └── storage/db.rs           # WAL SQLite migrations (messages, pending_messages,
                                    # sender_keys, seq_counters — server-side)
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
# Edit .env: GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET, JWT_SECRET, FRONTEND_URL

# Start server + client simultaneously
cd ../engage && make dev
```

### Option B — Docker Compose (server) + native client

```bash
cd engage && make docker-up   # relay server in container
make client                   # Tauri desktop client
```

### Option C — Manual

See step-by-step instructions below.

---

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Rust | ≥ 1.76 | Via [rustup](https://rustup.rs) |
| Node.js | ≥ 18.12 | Node 22 LTS via [nvm](https://github.com/nvm-sh/nvm) recommended |
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
- `._*` resource forks — on exFAT volumes run `find . -name "._*" -delete` before building
- `engage://` deep link — declared in `Info.plist` via `tauri.conf.json`; no runtime registration needed on macOS

### Windows toolchain

Targets `x86_64-pc-windows-gnu` — Visual Studio Build Tools **not** required.

```powershell
scoop install mingw nodejs-lts
corepack enable && corepack prepare pnpm@9.15.9 --activate
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup override set stable-x86_64-pc-windows-gnu   # inside src-tauri/
```

`.cargo/config.toml` applies `-fuse-ld=lld` automatically — resolves the Windows PE ordinal limit so `cdylib` can be included alongside `staticlib` and `rlib`.

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

**Consent scopes:** `openid` · `email` · `profile` · `https://www.googleapis.com/auth/contacts.readonly`

### 3. Configure and start the relay server

```bash
git clone --branch engage-server git@github.com:faridguzman91/rust-engage.git engage-server
cd engage-server && cp .env.example .env
cargo run
```

### 4. Start the client

```bash
pnpm install
pnpm tauri dev
```

### 5. First run

```
Launch → /login → "Continue with Google"
  └─► Google consent → JWT issued → /#/auth?token=JWT
        └─► /setup → enter display name → keys generated + registered
              └─► /chat — ready to message
```

---

## Configuration

### Frontend (`.env.local`)

| Variable | Desktop | Android emulator | Android device |
|---|---|---|---|
| `VITE_SERVER_URL` | `http://localhost:3000` | `http://10.0.2.2:3000` | `http://<LAN-IP>:3000` |

### Server (`.env`)

| Variable | Description |
|---|---|
| `GOOGLE_CLIENT_ID` | From Google Cloud Console |
| `GOOGLE_CLIENT_SECRET` | From Google Cloud Console |
| `JWT_SECRET` | `openssl rand -hex 32` |
| `FRONTEND_URL` | **Dev only** — `http://localhost:1420` |

Full reference: [engage-server/.env.example](https://github.com/faridguzman91/rust-engage/blob/engage-server/.env.example)

---

## Gmail contact import

```
Client → GET /api/contacts/suggest
  Server: load access_token → refresh if expired → GET People API
  Cross-reference emails vs. devices table (excludes self + unregistered)
  ← [{ userId, displayName, identityKey, email }]
User clicks "Add" → addContact(ik, name) → X3DH session init on next message
```

---

## Production build

```bash
pnpm tauri build
# Binaries → src-tauri/target/release/bundle/
```

Use HTTPS, remove `FRONTEND_URL` (falls back to `engage://` deep link), run server behind nginx/Caddy.

---

## Android build

The Android Gradle project (`src-tauri/gen/android/`) is already generated and committed. See [ANDROID.md](ANDROID.md) for the full guide. Quick version:

```bash
# 1. Install Android Studio + NDK 30, set ANDROID_HOME + NDK_HOME
# 2. Install Rust Android targets
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk --locked

# 3. Set server URL in .env.local (localhost won't reach from device)
echo "VITE_SERVER_URL=http://10.0.2.2:3000" >> .env.local   # emulator
# echo "VITE_SERVER_URL=http://192.168.x.x:3000" >> .env.local  # physical device

# 4. Enable Windows Developer Mode (Settings → System → For developers)
#    Required for symlink creation during APK packaging

# 5. Build APK
make android-build
# APK → src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk

# 6. Dev build with hot-reload on connected device / emulator
make android-dev
```

CI builds a debug APK on every push — artifacts available in GitHub Actions for 14 days.

---

## Tech stack

| Layer | Technology |
|---|---|
| Desktop + Android shell | [Tauri 2](https://tauri.app) |
| Frontend | [Vue 3](https://vuejs.org) + TypeScript |
| UI library | [PrimeVue 4](https://primevue.org) — Aura preset + PrimeIcons |
| State | [Pinia](https://pinia.vuejs.org) |
| Router | [Vue Router 4](https://router.vuejs.org) |
| QR codes | [qrcode](https://github.com/soldair/node-qrcode) |
| Packages | [pnpm](https://pnpm.io) |
| Build | [Vite](https://vitejs.dev) |
| Crypto (1:1) | X3DH + Double Ratchet — `x25519-dalek`, `ed25519-dalek`, `aes-gcm`, `hkdf` |
| Crypto (groups) | Sender Keys — AES-256-GCM + HKDF ratchet |
| Auth | Google OAuth 2.0 + HS256 JWT |
| Local DB | SQLite via [rusqlite](https://github.com/rusqlite/rusqlite) (bundled) |
| Relay server | [Axum 0.7](https://github.com/tokio-rs/axum) + Tokio |
| Containers | Docker + Docker Compose |

---

## Makefile targets

| Target | What it does |
|---|---|
| `make dev` | Start relay server + Tauri desktop client in parallel |
| `make server` | Start relay server only |
| `make client` | Start Tauri desktop client only |
| `make install` | Install / update frontend dependencies |
| `make build` | Production desktop build |
| `make android-init` | Re-generate `src-tauri/gen/android/` (only needed after `tauri.conf.json` changes) |
| `make android-dev` | Live-reload build on connected Android device or emulator |
| `make android-build` | Build debug APK |
| `make docker-up` | Start relay server via Docker Compose |
| `make docker-down` | Stop Docker Compose |
| `make clean` | Remove Rust + frontend build artefacts |

---

## Roadmap

- [x] **E2E encryption** — X3DH + Double Ratchet (forward secrecy, break-in recovery)
- [x] **Authentication** — Google OAuth 2.0 + HS256 JWT; all routes and WS connections protected
- [x] **Relay server** — zero-knowledge Axum server; stores and forwards sealed envelopes only
- [x] **Offline message drain** — messages queued server-side; delivered on reconnect
- [x] **OPK replenishment** — auto-upload 100 fresh OPKs when pool drops below 10
- [x] **PrimeVue UI** — Signal-inspired dark theme, PrimeVue 4 + Aura + PrimeIcons
- [x] **Disappearing messages** — per-conversation TTL; auto-delete on both sides
- [x] **Group messaging** — Sender Keys (Signal-style); one encrypt, server fans out
- [x] **Gmail contact import** — "Find from Gmail" via Google People API

### Message Reliability

- [x] **Delivery receipts** — `ack` forwarded from recipient to sender; `sent → delivered`
- [x] **Read receipts** — emitted when thread opened; `delivered → read`
- [x] **Retry queue** — `pending_messages` table; crash-safe; drained on every WS reconnect
- [x] **Sequence numbers** — `seq_counters` per recipient; gap detection triggers offline drain

### Invites

- [x] **Invite links** — `POST /api/invites` (24h token); `GET /api/invites/:token` (public); deep link via `onOpenUrl`
- [x] **QR code** — rendered in Settings with brand colours
- [x] **Share sheet** — copy, `mailto:`, `sms:` via `tauri-plugin-opener`

### Android Port

- [x] `cdylib` crate type re-enabled — Windows PE limit resolved via `rust-lld`
- [x] App Links — `https://engage.app/auth` + `/invite`; `android:autoVerify="true"` in `AndroidManifest.xml`
- [x] SQLite path — `app_data_dir()` resolves to `/data/data/com.engage.app/files/`
- [x] Android Gradle project generated and committed — `minSdkVersion=24`, INTERNET permission, edge-to-edge
- [x] Rust cross-compilation verified — all four ABIs compile successfully
- [x] **APK builds** — `app-universal-release-unsigned.apk` produced; Gradle warnings are all upstream Tauri / JDK version cosmetics, zero errors
- [x] CI pipeline — `.github/workflows/android.yml` (NDK 30, 4 ABI targets, debug APK artifact)
- [x] Makefile targets + rustup PATH fix for Windows Scoop installs
- [x] [`ANDROID.md`](ANDROID.md) — SDK/NDK setup, fingerprints (3 methods), OAuth client, `assetlinks.json`, server URL per target, signing, troubleshooting
- [ ] Set `VITE_SERVER_URL` for emulator (`10.0.2.2:3000`) or device (LAN IP) in `.env.local`
- [ ] Register Android OAuth 2.0 Client ID in Google Cloud Console with SHA-1 fingerprint
- [ ] Deploy `assetlinks.json` to `https://engage.app/.well-known/` with SHA-256 fingerprint for App Link verification

### iOS Port

- [ ] Rust targets: `aarch64-apple-ios`, `x86_64-apple-ios`, `aarch64-apple-ios-sim`
- [ ] `pnpm tauri ios init` → generate Xcode project under `src-tauri/gen/ios/`
- [ ] OAuth: `engage://` URL scheme reuses macOS `Info.plist` config
- [ ] Keychain: replace `localStorage` JWT storage with Tauri `stronghold` or `keychain` plugin
- [ ] Provisioning: Apple Developer account, bundle ID `app.engage.client`
- [ ] CI: `macos-latest` runner + `xcode-select`

### Voice / Video Calls (STUN/TURN NAT Traversal)

- [x] WS signaling — `CallOffer`, `CallAnswer`, `IceCandidate`, `CallHangup` in `WsEnvelope`; server routes all call frames as a pure relay (no SDP inspection)
- [x] `useWebRTC.ts` — full `RTCPeerConnection` composable; trickle ICE; caller/callee state machine (`idle → calling/ringing → active → ended`); mute + camera toggle
- [x] `CallView.vue` — pulsing incoming call overlay (Accept/Decline), active call view with remote video, local PiP video, call timer, mute/camera/hang-up controls, audio-only mode
- [x] Call buttons wired — `pi-phone` (voice) and `pi-video` (video) in `MessageThread` header trigger `rtc.startCall()`; disabled when call in progress
- [x] STUN — `stun:stun.l.google.com:19302` always included in ICE config
- [x] TURN credential endpoint — `GET /api/turn-credentials` issues HMAC-SHA1 short-term credentials (24h TTL, `lt-cred-mechanism`); gracefully omitted when `TURN_SECRET` not set
- [ ] Deploy `coturn` on VPS (`lt-cred-mechanism`, `realm=engage.app`, set `TURN_SECRET` env var)
- [ ] Media E2EE — AES-GCM insertable streams on top of mandatory DTLS-SRTP (post-coturn)

## Licence

Copyright (C) 2024–2026 **Farid Guzman** — [github.com/faridguzman91](https://github.com/faridguzman91)

This project is licensed under the **GNU Affero General Public License v3.0 (AGPLv3)**.

You are free to use, study, modify, and distribute engage under the terms of the AGPLv3. The key condition: **if you run a modified version of engage as a network service (e.g. a hosted chat platform), you must make your modifications available under the same licence.**

This means:
- ✅ Personal use, study, and self-hosting — always free
- ✅ Forking and contributing back — welcome
- ✅ Building on engage for open-source projects — allowed
- ❌ Forking and running as a closed-source commercial service — not permitted without a separate agreement

For commercial licensing enquiries (e.g. embedding engage in a proprietary product), contact the author via GitHub.

See the full licence text in [LICENSE](LICENSE).

---

### Microservices Decomposition

- [ ] `auth-svc` — Google OAuth, JWT issue/verify, token refresh
- [ ] `key-svc` — identity key storage, OPK distribution; migrate to Postgres
- [ ] `relay-svc` — message store-and-forward; back with Redis queue
- [ ] `ws-svc` — WebSocket connections, push delivery, presence; Redis pub/sub
- [ ] `group-svc` — group CRUD, Sender Key distribution, fan-out
- [ ] `turn-svc` — TURN credential issuance
- [ ] API gateway — JWT middleware + service routing (nginx or Axum tower)
- [ ] Migration path — `auth-svc` first → `key-svc` → `relay-svc` + `ws-svc` together
