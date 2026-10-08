# wobook Android

## Stack

- Kotlin, Jetpack Compose (Material 3), Navigation, DataStore, WorkManager
- CameraX + zxing for QR pairing
- Rust core (`crates/wobook-ffi`), built with `cargo-ndk` and bound to Kotlin with UniFFI (JNA)
- minSdk 31, targetSdk 36, ABIs `arm64-v8a` and `x86_64`
- Nix devShells provide the SDK, NDK, JDK 21, Rust and adb; Maestro handles e2e

## Dev

Run these from the repo root. On the phone, turn on USB debugging (Developer options), then plug it in.

```bash
nix develop .#android
cd android
adb devices                                          # phone should be listed
./gradlew installDebug -Pwobook.rustProfile=debug    # fast Rust build, debug APK
adb shell am start -n dev.wochap.wobook/.MainActivity
adb logcat --pid=$(adb shell pidof dev.wochap.wobook)
```

Without the property, the Rust lib is built with `--release`.

Tests:

```bash
./gradlew test                                       # unit tests
nix develop .#android-emulator -c android/scripts/e2e.sh   # Maestro on headless emulator + desktop wobookd
```

## Prod

```bash
nix develop .#android
cd android
./gradlew assembleRelease            # app/build/outputs/apk/release/app-release.apk
adb install -r app/build/outputs/apk/release/app-release.apk
```

The release build is signed with the debug key, which is fine for sideloading. Because both builds use the same key, `install -r` upgrades over a debug install.
To install without adb, copy the APK to the phone, open it, and allow "Install unknown apps" for your file manager.

## How it works

```
Compose UI ──► AppRepository (Kotlin facade) ──► WobookApp (Rust via UniFFI)
   ▲                                              │  wobook-core: Automerge doc, URL-keyed bookmarks
   └──── AppListener callbacks (revision, ◄───────┤  wobook-sync: P2P sync with paired peers
         sync status, devices, pairing)           ▼
                                           app files dir (docs + control db)
```

- **Rust is the source of truth.** Kotlin keeps no cache. It holds a revision counter and re-queries whenever the listener reports a change.
- **Pairing:** one device shows a QR code or pasteable payload (`domain/QrPayload.kt`) and the other scans or pastes it. After that the peers trust each other's keys (`crypto/KeystoreSecureStore.kt`).
- **Sync:** runs while the app is in the foreground (`ForegroundSyncLifecycle`), with periodic background syncs through WorkManager (`BackgroundSyncWorker`). It talks directly to desktop `wobookd` and other phones, with no relay.
- **Share target:** `ShareReceiverActivity` takes URLs that other apps share and opens the add sheet.
