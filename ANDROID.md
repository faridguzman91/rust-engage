# Android Port — Developer Setup Guide

> @faridguzman: Step-by-step instructions for building and running engage on Android.
> The Tauri 2 Android target compiles the same Rust crypto core and Vue 3 frontend
> that power the desktop app — no separate codebase required.

---

## Prerequisites overview

| Tool | Version | Purpose |
|---|---|---|
| Android Studio / SDK | Ladybug (2024.2.1+) | SDK Manager, emulator |
| Android NDK | 27.x | Cross-compiling Rust → Android ABIs |
| Java JDK | 17 | Gradle build system |
| Rust | stable | Cross-compilation targets |
| `cargo-ndk` | latest | Tauri's Android build uses it internally |
| Node 22 LTS + pnpm 9 | — | Frontend build (same as desktop) |

---

## 1. Install Android Studio and SDK

Download from https://developer.android.com/studio and run the installer.

During first-launch setup, make sure the following SDK components are installed via **SDK Manager → SDK Platforms / SDK Tools**:

| Component | Required version |
|---|---|
| Android SDK Platform | API 35 (Android 15) |
| Android SDK Build-Tools | 35.0.0 |
| Android NDK (Side by side) | 27.x |
| Android SDK Command-line Tools | latest |

Then set the environment variables in your shell profile (`~/.bashrc`, `~/.zshrc`, or Windows environment settings):

**macOS / Linux**
```bash
export ANDROID_HOME="$HOME/Library/Android/sdk"          # macOS default
# export ANDROID_HOME="$HOME/Android/Sdk"                # Linux default
export NDK_HOME="$ANDROID_HOME/ndk/27.0.11902837"
export PATH="$ANDROID_HOME/tools:$ANDROID_HOME/platform-tools:$PATH"
```

**Windows (PowerShell profile)**
```powershell
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:NDK_HOME     = "$env:ANDROID_HOME\ndk\27.0.11902837"
$env:PATH         = "$env:ANDROID_HOME\platform-tools;$env:PATH"
```

Verify:
```bash
adb version          # Android Debug Bridge
sdkmanager --list    # confirms SDK tools found
```

---

## 2. Install Rust Android targets

```bash
rustup target add aarch64-linux-android      # modern arm64 phones (required)
rustup target add armv7-linux-androideabi    # older arm32 devices
rustup target add x86_64-linux-android       # x86_64 emulators
rustup target add i686-linux-android         # x86 emulators (optional)
```

Verify:
```bash
rustup target list --installed | grep android
```

---

## 3. Install cargo-ndk

Tauri's Android build pipeline uses `cargo-ndk` to cross-compile the Rust library for each ABI.

```bash
cargo install cargo-ndk --locked
```

---

## 4. Generate the Android Gradle project (one-time)

This step reads `tauri.conf.json` and generates `src-tauri/gen/android/` — a full Android Studio project with Gradle build files, `AndroidManifest.xml`, and the JNI glue.

```bash
make android-init
# or directly:
pnpm tauri android init
```

> **Note:** Commit the generated `src-tauri/gen/android/` directory. Subsequent `android-init` runs are safe (idempotent) but only needed if you update `tauri.conf.json` bundle settings.

---

## 5. Deep-link / OAuth on Android

Desktop engage uses the `engage://` custom URI scheme. Android uses **App Links** (verified HTTPS links) instead, which are more secure and require domain verification.

The mobile deep-link is configured in `tauri.conf.json`:
```json
"mobile": [
  {
    "host": "engage.app",
    "pathPrefix": ["/auth", "/invite"],
    "scheme": "https"
  }
]
```

For production, add a `.well-known/assetlinks.json` file to `https://engage.app/.well-known/assetlinks.json` with your app's certificate fingerprint:

```json
[{
  "relation": ["delegate_permission/common.handle_all_urls"],
  "target": {
    "namespace": "android_app",
    "package_name": "com.engage.app",
    "sha256_cert_fingerprints": ["YOUR_SIGNING_CERT_SHA256"]
  }
}]
```

Get the fingerprint from your keystore:
```bash
keytool -list -v -keystore release.keystore -alias engage
```

For **development**, the relay server's Google OAuth callback can redirect to `http://10.0.2.2:1420/#/auth?token=JWT` (the Android emulator's alias for the host machine's localhost) instead of the App Link.

---

## 6. Run on a device or emulator

### Physical device
1. Enable **Developer Options → USB Debugging** on the device
2. Connect via USB
3. Verify: `adb devices` shows your device as `authorized`

### Emulator
Create an AVD in Android Studio (**Device Manager → Create Virtual Device**).
Use a Pixel 7 profile with API 35 (arm64-v8a) for the closest match to real hardware.

### Start the dev server + Android hot-reload
```bash
# Terminal 1 — relay server
make server

# Terminal 2 — Android dev build (connects to server via LAN IP)
make android-dev
```

> Tauri will ask which device/emulator to target if more than one is connected.

---

## 7. Build a release APK

```bash
make android-build
# APK output: src-tauri/gen/android/app/build/outputs/apk/release/
```

For a signed release APK suitable for the Play Store:

```bash
# Generate a keystore (one-time)
keytool -genkey -v \
  -keystore release.keystore \
  -alias engage \
  -keyalg RSA -keysize 4096 \
  -validity 10000

# Build and sign
pnpm tauri android build --apk \
  -- -Pandroid.injected.signing.store.file=$(pwd)/release.keystore \
     -Pandroid.injected.signing.store.password=YOUR_STORE_PASS \
     -Pandroid.injected.signing.key.alias=engage \
     -Pandroid.injected.signing.key.password=YOUR_KEY_PASS
```

---

## 8. CI — GitHub Actions

The `.github/workflows/android.yml` workflow builds a **debug APK** on every push to `master` and every pull request.

**What it installs:**
- Temurin JDK 17
- Android SDK + NDK 27 via `android-actions/setup-android`
- Rust stable + all four Android targets
- `cargo-ndk`
- Node 22 + pnpm 9
- Frontend dependencies via `pnpm install --frozen-lockfile`

**Artifact:** `engage-android-debug-<sha>` — downloadable from the Actions run for 14 days.

---

## 9. SQLite path

On Android, `app_data_dir()` (already used in `lib.rs`) resolves to:
```
/data/data/com.engage.app/files/engage.db
```

This path is private to the app, backed up by Android Auto Backup by default, and not accessible to other apps without root. No code change required.

---

## 10. Troubleshooting

| Error | Fix |
|---|---|
| `ANDROID_HOME is not set` | Export the variable in your shell profile and restart the terminal |
| `NDK_HOME is not set` | Point to the exact NDK version directory (not just `$ANDROID_HOME/ndk/`) |
| `error: linker 'aarch64-linux-android-clang' not found` | Ensure NDK_HOME is correct; `cargo-ndk` constructs the linker path from it |
| `Could not determine the dependencies of task ':app:compileDebugJavaWithJavac'` | Run `pnpm tauri android init` again after a `tauri.conf.json` change |
| App crashes on launch | Run `adb logcat -s RustStdoutStderr:V` to see Rust `eprintln!` / `tracing` output |
| OAuth redirect does not open the app | Confirm `assetlinks.json` is served over HTTPS with correct fingerprint |
