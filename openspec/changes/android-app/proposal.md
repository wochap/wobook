## Why

The phone is the reason wobook exists: with buku the Android device could only read a Syncthing copy and never save. After `core-and-daemon` and `p2p-sync`, every Linux machine is a full peer; this change makes the Android phone one too. A native Kotlin app saves links from the share sheet in seconds, fuzzy-searches the library offline, edits and deletes, pairs by QR and syncs peer-to-peer with the desktops. No hub, no cloud.

## What Changes

- New crate `crates/wobook-ffi`: UniFFI bindings over `wobook-core` and `wobook-sync` so the phone runs the same Automerge document, projection, search, metadata fetch, identity, pairing and QUIC transport as the desktop daemon.
- New `android/` Gradle project (Kotlin 2.x, Jetpack Compose Material 3, minSdk 31, package `dev.wochap.wobook`) that builds the Rust library with `cargo-ndk` for `arm64-v8a` and `x86_64`, generates Kotlin bindings with `uniffi-bindgen`, and ships them in the APK.
- UI implements the Claude Design handoff in `design/project/` (45 frames, Catppuccin Mocha/Latte tokens, Inter + JetBrains Mono, Phosphor icons): Home/search, Detail, Add/Edit form, Share-sheet receiver, Pairing (scan, show QR, confirm), Devices, Settings, Onboarding.
- Android Keystore-backed implementation of the `SecureKeyStore` trait (AES-GCM wrapping key, AAD `wobook:<kind>:v1`) for the device identity key.
- Sync lifecycle: full peer while the app is in the foreground (multicast lock, QUIC listener, mDNS), WorkManager periodic sync (15 min, unmetered) in the background, no persistent foreground service.
- Nix flake gains `android` and `android-emulator` dev shells (copied from session-tap) alongside the existing Rust shell.
- Maestro end-to-end flows under `android/maestro/` driven on the Nix emulator against a desktop `wobookd` reachable at `10.0.2.2`.

## Capabilities

### New Capabilities
- `ffi-bindings`: UniFFI surface of `wobook-ffi` (lifecycle, bookmarks, search, tags, fetch, import/export, identity, pairing, devices, sync status, callbacks) and its Android build pipeline.
- `android-shell`: app skeleton, theme tokens mapped to Compose, navigation, onboarding, settings, large-screen and dynamic-type rules.
- `android-search-home`: Home screen search, highlighting, tag chip filtering, result row actions, empty/no-result/syncing states.
- `android-bookmark-editing`: Detail screen, Add/Edit form with TagEditor and fetch states, URL change as rename, delete with undo.
- `android-share-receiver`: share-sheet bottom sheet, duplicate handling, fetch states, offline behaviour, post-save confirmation.
- `android-pairing-ui`: scan, show-my-QR, fingerprint confirmation, error states, paste fallback.
- `android-devices-sync`: Devices screen, reachability, rename/revoke, foreground sync, background WorkManager sync, sync status surfaces.
- `android-e2e-testing`: Maestro flows, emulator shell, desktop peer fixture, unit tests limited to pure domain code.

### Modified Capabilities
<!-- none: the secure key store requirement is added under ffi-bindings; p2p-sync's own specs are untouched -->

## Impact

- New code: `crates/wobook-ffi`, `android/`, `android/maestro/`, flake dev shells. Desktop binaries unchanged.
- `wobook-core` and `wobook-sync` public APIs must be FFI-friendly: owned types, no lifetimes, `Send + Sync` handles, errors as enums. Small refactors allowed, no behaviour change.
- New external dependencies: `uniffi` (Rust + Gradle plugin or bindgen CLI), `cargo-ndk`, AndroidX Compose BOM, CameraX, zxing-cpp, WorkManager, DataStore, Phosphor icons, Maestro CLI.
- Design bundle `design/project/wobook.dc.html` is the visual contract; the coding agent reads it in full before building screens.
- Release builds signed with the debug key; sideload only. Alpha.
