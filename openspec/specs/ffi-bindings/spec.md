# ffi-bindings Specification

## Purpose
UniFFI surface of `wobook-ffi` (lifecycle, bookmarks, search, tags, fetch, import/export, identity, pairing, devices, sync status, callbacks) and its Android build pipeline.

## Requirements

### Requirement: Single application object over the FFI
`wobook-ffi` SHALL expose a `WobookApp` object created with an `AppConfig` (data dir, device name, network enabled, tailnet allowed), a `SecureKeyStore` callback and an `AppListener` callback. The object SHALL own the Automerge repository, the read model, the identity and the sync transport, and SHALL be the only writer for its data directory within the process.

#### Scenario: Open on a fresh directory
- **WHEN** `WobookApp.open` is called on an empty data directory with an in-memory key store
- **THEN** it initializes a new document, creates a device identity through the key store and returns an object whose `list` is empty

#### Scenario: Reopen keeps data
- **WHEN** a bookmark was added, the object is shut down and `open` is called again on the same directory
- **THEN** `list` returns the bookmark

### Requirement: Bookmark operations mirror the daemon API
The FFI SHALL expose `add`, `update`, `rename`, `delete`, `restore`, `get`, `list`, `search`, `tags`, `fetch_metadata`, `import`, `export`, `normalize_url` and `parse_tags` with the same normalization, merge, tombstone and error semantics as the `daemon-api` capability.

#### Scenario: Add existing with merge
- **WHEN** `add` is called for an existing URL with `merge = true` and new tags
- **THEN** the result has `merged = true` and the bookmark's tags are the union

#### Scenario: Error surfaced as typed enum
- **WHEN** `get` is called with an invalid URL
- **THEN** the call fails with `WobookError.InvalidUrl` carrying a message

### Requirement: Search hits carry per-field highlight indices
`search` SHALL return hits with `title_indices` and `url_indices` (character offsets within the title and the displayed URL) derived from the core haystack indices.

#### Scenario: Highlight positions
- **WHEN** the query `shcn` matches `shadcn/ui`
- **THEN** `title_indices` contains the offsets of `s`, `h`, `c`, `n` within the title and every index is below the title length

### Requirement: Secure key store is injectable
The identity private key and discovery secrets SHALL be stored through a `SecureKeyStore` interface with `load(kind)`, `store(kind, bytes)` and `remove(kind)`, where `kind` is one of `device_key`, `discovery_secret`, `discovery_secret_prev`. The Rust side SHALL never write key material to its data directory when a key store is provided.

#### Scenario: Key created through the callback
- **WHEN** `open` runs with a key store that has no `device_key`
- **THEN** `store("device_key", ...)` is called exactly once and later opens call `load("device_key")` without storing again

#### Scenario: Corrupt key material
- **WHEN** `load("device_key")` fails with a corrupt error
- **THEN** `open` fails with `WobookError.IdentityLost` and does not create a new identity silently

### Requirement: Events delivered through the listener
The FFI SHALL call `on_data_changed` after any local or remote document change, `on_sync_status` on sync state transitions and `on_pairing_event` for pairing progress, each from a background thread, never blocking Rust work on the callback's duration.

#### Scenario: Remote change notifies
- **WHEN** a peer syncs a new bookmark into the phone
- **THEN** `on_data_changed` fires and `list` includes the bookmark

### Requirement: Pairing, devices and sync control
The FFI SHALL expose `start_pairing_offer`, `join_pairing`, `pending_confirmations`, `confirm_pairing`, `devices`, `this_device`, `rename_device`, `revoke_device`, `sync_status`, `sync_now`, `set_foreground` and `shutdown`, delegating to `wobook-sync`.

#### Scenario: Offer payload is valid JSON
- **WHEN** `start_pairing_offer` is called
- **THEN** the returned payload parses as the `p2p-sync` QR payload with version, id, endpoints, secret and expiry, and `expires_at_ms` is about 120 seconds ahead

#### Scenario: Foreground toggles networking
- **WHEN** `set_foreground(false)` is called
- **THEN** discovery stops and peer connections close; `set_foreground(true)` restarts discovery and reconnects known endpoints

### Requirement: Android build pipeline
The Android Gradle build SHALL compile `wobook-ffi` with `cargo-ndk` for `arm64-v8a` and `x86_64` into `jniLibs`, generate Kotlin bindings with the crate's own `uniffi-bindgen` binary into a generated source set, and SHALL fail the build if either step fails. Generated bindings SHALL not be committed.

#### Scenario: Debug build from the Nix shell
- **WHEN** `./gradlew assembleDebug` runs inside `nix develop .#android`
- **THEN** the APK contains `lib/arm64-v8a/libwobook_ffi.so` and `lib/x86_64/libwobook_ffi.so` and the Kotlin bindings compile

### Requirement: FFI smoke test
`wobook-ffi` SHALL have a Rust test that opens a `WobookApp` on a temporary directory with in-memory key store and listener, adds and searches a bookmark and starts a pairing offer.

#### Scenario: cargo test
- **WHEN** `cargo test -p wobook-ffi` runs
- **THEN** the smoke test passes without network access
