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
export NDK_HOME="$ANDROID_HOME/ndk/30.0.14904198"
export PATH="$ANDROID_HOME/tools:$ANDROID_HOME/platform-tools:$PATH"
```

**Windows (PowerShell profile)**
```powershell
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:NDK_HOME     = "$env:ANDROID_HOME\ndk\30.0.14904198"
$env:PATH         = "$env:ANDROID_HOME\platform-tools;$env:PATH"
```

> **Scoop users — rustup PATH fix:** If you installed Rust via `scoop install rust` and also have `scoop install rustup`, the `rustup.exe` binary may not be on PATH (scoop only shims one cargo). `pnpm tauri android` needs `rustup` to install cross-compilation targets. The Makefile already prepends the correct path automatically. If running Tauri commands directly from PowerShell, prepend it manually:
> ```powershell
> $env:PATH = "$env:USERPROFILE\scoop\apps\rustup\current\.cargo\bin;$env:PATH"
> pnpm tauri android init
> ```

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

## 5. Get your SHA-256 certificate fingerprint

Both Google Sign-In (OAuth) and Android App Links require your app's SHA-256 signing certificate fingerprint. There are three ways to get it depending on your workflow.

### Method A — Android Studio Gradle signingReport (recommended)

> @faridguzman: This is the easiest method once the Android project has been generated.

1. Open the Android project in Android Studio:
   `C:\Users\farid\StudioProjects\rust-engage\src-tauri\gen\android`
2. Open the **Gradle** panel (right-hand side toolbar)
3. Navigate to **engage → app → Tasks → android → signingReport**
4. Double-click **signingReport** — fingerprints for all build variants (debug + release) are printed in the Run console

---

### Method B — keytool via Android Studio JBR (Windows)

Android Studio ships its own JBR (Java Bundle Runtime) with a bundled `keytool`. Use this instead of a system Java installation to guarantee version compatibility.

#### Debug fingerprint (development / OAuth testing)

The debug keystore is auto-generated the first time you run `make android-dev` or `make android-build`.

```powershell
# Run make android-dev first if debug.keystore doesn't exist yet
& "C:\Program Files\Android\Android Studio\jbr\bin\keytool.exe" `
    -list -v `
    -keystore "$HOME\.android\debug.keystore" `
    -alias androiddebugkey `
    -storepass android
```

Look for the line starting with `SHA256:` in the output — that is your debug fingerprint.

#### Release fingerprint (production / Play Store)

Generate a release keystore if you don't have one yet:

```powershell
# Generate keystore (one-time — keep this file safe and out of version control)
& "C:\Program Files\Android\Android Studio\jbr\bin\keytool.exe" `
    -genkey -v `
    -keystore release.keystore `
    -alias engage `
    -keyalg RSA -keysize 4096 `
    -validity 10000

# Print the fingerprint
& "C:\Program Files\Android\Android Studio\jbr\bin\keytool.exe" `
    -list -v `
    -keystore release.keystore `
    -alias engage
```

---

### Method C — keytool on macOS / Linux

```bash
# Debug
keytool -list -v \
  -keystore ~/.android/debug.keystore \
  -alias androiddebugkey \
  -storepass android

# Release
keytool -list -v -keystore release.keystore -alias engage
```

---

## 6. Register the fingerprint in two places

Once you have the SHA-256 fingerprint you need to register it in two places.

### 6a. Google Cloud Console — Android OAuth client

Google Sign-In on Android requires a separate **Android** OAuth 2.0 Client ID (in addition to the Web client used by the relay server).

1. Go to [Google Cloud Console](https://console.cloud.google.com/) → **APIs & Services → Credentials**
2. Click **+ Create Credentials → OAuth 2.0 Client ID**
3. Application type: **Android**
4. Package name: `com.engage.app`
5. SHA-1 certificate fingerprint: paste the `SHA1:` value from `keytool` output
   *(Google's Android OAuth requires SHA-1, not SHA-256)*
6. Save — no client secret is used for Android OAuth

> **Debug vs release:** Create one Android client for the debug keystore (for local development) and a second for the release keystore (for Play Store builds). Both can coexist under the same Google Cloud project.

### 6b. assetlinks.json — Android App Link verification

App Links (`https://engage.app/auth`, `https://engage.app/invite`) require a verification file served from your domain. Create/update `https://engage.app/.well-known/assetlinks.json`:

```json
[{
  "relation": ["delegate_permission/common.handle_all_urls"],
  "target": {
    "namespace": "android_app",
    "package_name": "com.engage.app",
    "sha256_cert_fingerprints": [
      "AA:BB:CC:...:YOUR_DEBUG_SHA256",
      "DD:EE:FF:...:YOUR_RELEASE_SHA256"
    ]
  }
}]
```

You can include multiple fingerprints in the array — list both debug and release so both builds intercept the links.

Verify the file is reachable and valid:
```bash
# From the device or emulator
adb shell am start -a android.intent.action.VIEW \
  -d "https://engage.app/invite?token=test" com.engage.app

# Or use Google's Digital Asset Links validator:
# https://developers.google.com/digital-asset-links/tools/generator
```

For **local development without a live domain**, the relay server's Google OAuth callback can redirect to `http://10.0.2.2:1420/#/auth?token=JWT` (the Android emulator's alias for the host machine's `localhost`) instead of the App Link — no `assetlinks.json` needed for dev.

---

## 7. Run on a device or emulator

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

## 8. Build a release APK

```bash
make android-build
# APK output: src-tauri/gen/android/app/build/outputs/apk/release/
```

For a signed release APK suitable for the Play Store, pass signing parameters to Gradle.

**Windows (PowerShell)**
```powershell
pnpm tauri android build --apk -- `
  "-Pandroid.injected.signing.store.file=$PWD\release.keystore" `
  "-Pandroid.injected.signing.store.password=YOUR_STORE_PASS" `
  "-Pandroid.injected.signing.key.alias=engage" `
  "-Pandroid.injected.signing.key.password=YOUR_KEY_PASS"
```

**macOS / Linux**
```bash
pnpm tauri android build --apk \
  -- -Pandroid.injected.signing.store.file=$(pwd)/release.keystore \
     -Pandroid.injected.signing.store.password=YOUR_STORE_PASS \
     -Pandroid.injected.signing.key.alias=engage \
     -Pandroid.injected.signing.key.password=YOUR_KEY_PASS
```

> **Never commit `release.keystore` or passwords to the repository.** Add `release.keystore` to `.gitignore` and store credentials in a secrets manager or CI secrets.

---

## 9. CI — GitHub Actions

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

## 10. SQLite path

On Android, `app_data_dir()` (already used in `lib.rs`) resolves to:
```
/data/data/com.engage.app/files/engage.db
```

This path is private to the app, backed up by Android Auto Backup by default, and not accessible to other apps without root. No code change required.

---

## 11. Troubleshooting

| Error | Fix |
|---|---|
| `ANDROID_HOME is not set` | Export the variable in your shell profile and restart the terminal |
| `NDK_HOME is not set` | Point to the exact NDK version directory (not just `$ANDROID_HOME/ndk/`) |
| `error: linker 'aarch64-linux-android-clang' not found` | Ensure NDK_HOME is correct; `cargo-ndk` constructs the linker path from it |
| `Could not determine the dependencies of task ':app:compileDebugJavaWithJavac'` | Run `pnpm tauri android init` again after a `tauri.conf.json` change |
| App crashes on launch | Run `adb logcat -s RustStdoutStderr:V` to see Rust `eprintln!` / `tracing` output |
| OAuth redirect does not open the app | Confirm `assetlinks.json` is served over HTTPS with correct fingerprint |
| Google Sign-In fails on device | Add an **Android** OAuth 2.0 Client ID in Google Cloud Console with the device's SHA-1 fingerprint |
| `debug.keystore` not found | Run `make android-dev` once — Gradle auto-generates it in `%USERPROFILE%\.android\` |
| signingReport shows wrong SHA | Make sure you ran signingReport on the correct build variant (debug vs release) |
