## Context

After `core-and-daemon` the Rust workspace has `automerge_repo`, `wobook-core` (domain, `doc` functions, projection, nucleo search, fetch, interchange) and the `wobookd`/`wobook` binaries. After `p2p-sync` it has `wobook-sync` (Ed25519 identity, trust store, QR pairing with HMAC proof, QUIC transport, mDNS, endpoint memory with LAN + Tailscale addresses). The daemon drives `automerge_repo::Repo` with the real transport.

The phone must be the same kind of peer, so it embeds those crates directly. Kotlin talks to them over UniFFI; nothing is hand-mirrored (the session-tap Android app mirrored protocol types by hand, which this design avoids).

Reference code:
- `/home/gean/Sandboxes/sandbox/session-tap/android/`: Gradle project layout, `gradle/libs.versions.toml`, package layout `crypto/ net/ data/ domain/ service/ ui/`, `ui/pairing/ScanScreen.kt` (CameraX + zxing-cpp), `domain/QrValidation.kt`, `scripts/emulator.sh`.
- `/home/gean/Sandboxes/sandbox/session-tap/flake.nix` lines 12-45: `androidComposition`, `androidShell`, `android` and `android-emulator` dev shells.
- `/home/gean/Sandboxes/sandbox/fi/flutter_app/android/app/src/main/kotlin/com/wochap/fi/MainActivity.kt`: Android Keystore AES-GCM wrapping (lines 158-247) and multicast lock handling (lines 140-155).
- `/home/gean/Sandboxes/sandbox/fi/crates/app_core/src/identity.rs` line 187: `SecureKeyStore` trait shape.
- Visual contract: `design/project/wobook.dc.html`, `design/project/readme.md` (token-to-Compose mapping, component list), `design/project/wobook-tokens.css`. Read the HTML in full; the readme's "Tokens → Compose" and "Components" sections are normative.

## Goals / Non-Goals

**Goals:**
- Save from the share sheet in under five seconds, fully offline.
- Fuzzy search identical to the desktop (same nucleo code).
- Full peer: Automerge repo, QUIC, pairing and trust all run on the phone.
- Pixel-faithful implementation of the 45 design frames in both themes.
- Maestro e2e on the Nix emulator against a real desktop daemon.

**Non-Goals:**
- Hub or relay server, iOS, tablets beyond the single-column rule, widgets, quick-settings tile, tag management screen, folders, favorites, accounts.
- Persistent foreground service or push notifications.
- Play Store signing, release pipeline.
- Camera-based pairing in automated tests (paste path only).

## Decisions

### D1. wobook-ffi crate and UniFFI style

`crates/wobook-ffi` is a `cdylib` + `rlib` named `libwobook_ffi.so`, using UniFFI proc-macros (`#[uniffi::export]`, `#[derive(uniffi::Record)]`, `#[derive(uniffi::Enum)]`, `#[derive(uniffi::Object)]`, `#[uniffi::export(callback_interface)]`), `uniffi::setup_scaffolding!()`, and a `uniffi-bindgen` binary target (`src/bin/uniffi-bindgen.rs` calling `uniffi::uniffi_bindgen_main()`) so Gradle can generate Kotlin without a separate install. Async Rust functions are exported as Kotlin `suspend` functions (UniFFI async support); the Tokio runtime lives inside the `WobookApp` object.

Public surface (all types owned, `Send + Sync`, errors as `#[derive(uniffi::Error)]` enums with a `message` field):

```
WobookApp::open(config: AppConfig, key_store: Box<dyn SecureKeyStore>, listener: Box<dyn AppListener>) -> WobookApp
  AppConfig { data_dir, device_name, enable_network: bool, allow_tailnet: bool }
  SecureKeyStore (callback interface): load(kind) -> Option<Vec<u8>>, store(kind, bytes), remove(kind)   // kinds: "device_key", "discovery_secret", "discovery_secret_prev"
  AppListener (callback interface): on_data_changed(DataChange), on_sync_status(SyncStatus), on_pairing_event(PairingEvent)

Bookmarks (same semantics as daemon-api):
  add(AddRequest) -> AddResult { bookmark, merged, fetch: FetchOutcome }
  update(UpdateRequest), rename(from, to), delete(url), restore(url), get(url) -> Option<Bookmark>
  list(ListQuery) -> Vec<Bookmark>, search(SearchQuery) -> Vec<Hit { bookmark, score, title_indices, url_indices }>
  tags() -> Vec<TagCount>, fetch_metadata(url) -> Metadata
  import(format, path) -> ImportReport, export(format, path, include_deleted)
  normalize_url(input) -> Result<String>, parse_tags(input) -> Vec<String>

Devices and sync:
  this_device() -> Device, devices() -> Vec<DeviceView { id, name, platform, reachability: Lan|Tailnet|Unreachable, last_synced_ms, syncing }>
  rename_device(id, name), revoke_device(id)
  sync_status() -> SyncStatus { state: UpToDate|Syncing(device_name)|NoPeerReachable|Disabled, last_sync_ms }
  sync_now(), set_foreground(bool), shutdown()

Pairing (wobook-sync state machine):
  start_pairing_offer() -> PairingOffer { qr_payload_json, expires_at_ms }       // "Show my QR"
  join_pairing(payload_json) -> JoinHandle(id)                                    // from scan or paste; validates payload first
  pending_confirmations() -> Vec<PairingConfirmation { id, peer_name, peer_platform, fingerprint_groups: [String;4] }>
  confirm_pairing(id, trust: bool)
  PairingEvent: Connecting{peer, via}, ConfirmRequired(PairingConfirmation), Completed(device), Expired, Rejected, Unreachable{tried: Vec<String>}, Failed(message)
```
`Hit.title_indices`/`url_indices` are derived in Rust from the haystack indices and segment boundaries (`fuzzy-search` spec) so Kotlin only draws spans.

Alternative considered: Kotlin client over a WebSocket to a desktop hub (session-tap model). Rejected: phone would not work offline and needs a hub; contradicts peer-to-peer decision.

### D2. Build pipeline

`android/app/build.gradle.kts` adds a `cargoNdkBuild` task before `preBuild`:
1. `cargo ndk -t arm64-v8a -t x86_64 -o app/src/main/jniLibs build --release -p wobook-ffi` (debug variant uses debug profile), run from the repo root with `ANDROID_NDK_HOME` from the Nix shell.
2. `cargo run -p wobook-ffi --bin uniffi-bindgen generate --library target/<triple>/release/libwobook_ffi.so --language kotlin --out-dir app/build/generated/uniffi` added as a Kotlin source set. Generated code is not committed.
The flake `android` shell provides `cargo-ndk`, the NDK (`includeNDK = true`, pinned version in `flake.nix`), JDK 21, Android SDK 36, build-tools 36.0.0; `android-emulator` adds the x86_64 `google_apis` system image, emulator, `maestro`, `adb`, and the Rust toolchain so `wobookd` can be built as the desktop fixture. Session-tap's `androidShell` function is copied and extended; the existing Rust shell from `core-and-daemon` stays the default.

### D3. App architecture (Kotlin)

```
android/app/src/main/java/dev/wochap/wobook/
  WobookApplication.kt      owns one WobookApp (Rust) for the process; opens it lazily on first use
  crypto/KeystoreSecureStore.kt   SecureKeyStore impl: AndroidKeyStore AES-GCM key "wobook-wrap", wraps secrets, blob stored in app-private files, AAD "wobook:<kind>:v1" (fi pattern)
  data/AppRepository.kt     thin Kotlin facade over WobookApp exposing StateFlow<List<Bookmark>> (refreshed on on_data_changed), SyncStatus flow, devices flow, pairing events flow
  data/Settings.kt          DataStore prefs: device name set, background sync on/off, tap behaviour (Detail|Open), auto-fetch on/off, onboarding done
  domain/                   pure: QrPayload validation (mirrors wobook-sync rules, used before calling join_pairing for instant UI errors), TagEditor commit rules, UrlDisplay (host+path formatting)
  service/ForegroundSyncLifecycle.kt   ProcessLifecycleOwner observer: onStart -> multicast lock + set_foreground(true); onStop -> set_foreground(false) + release
  service/BackgroundSyncWorker.kt      WorkManager CoroutineWorker: open app, set_foreground(true), sync_now(), wait until status is UpToDate or 60 s, set_foreground(false); periodic 15 min, NetworkType.UNMETERED, enabled by setting
  ui/theme/                 Tokens.kt (Mocha + Latte color sets, WobookColors CompositionLocal for success/tailscale/warning), Type.kt (Inter, JetBrains Mono), Shapes.kt, from design readme
  ui/home/ ui/detail/ ui/form/ ui/share/ ui/pairing/ ui/devices/ ui/settings/ ui/onboarding/ ui/components/
  ShareReceiverActivity.kt  separate translucent activity (theme Theme.Translucent.NoTitleBar), ACTION_SEND text/plain, hosts ShareReceiverSheet
```
No Room database: the Rust read model is the cache. Kotlin holds in-memory lists refreshed from Rust on `on_data_changed`; search calls Rust on every keystroke debounced at 60 ms (nucleo over thousands of rows is sub-millisecond).

Alternative: Room mirror like session-tap. Rejected: two caches of the same data, and Rust already has SQLite.

### D4. Navigation and screens

Single-activity Compose Navigation: `onboarding/name`, `onboarding/choose`, `home`, `detail/{url}`, `form?url=&new=`, `pairing/scan`, `pairing/show`, `pairing/confirm/{id}`, `pairing/result`, `devices`, `settings`. Predictive back enabled. Edge-to-edge with `enableEdgeToEdge()`. Each design frame id maps to a composable state; the frame label text is reused as the Compose `testTag` prefix (e.g. `home-results`) so Maestro selectors are stable.

Design rules carried over verbatim from `design/project/readme.md`: outlined buttons, accent only for lines/highlights/matched characters, Phosphor Regular 24/22/16, chips 32 dp, rows min 64 dp, fields 52 dp, FAB hidden when IME visible, delete via snackbar undo (6 s) never a dialog, revoke is the only AlertDialog, offline renders identical to online, 640 dp max content width centred, all type in `sp`.

### D5. TagEditor rules

Commit a chip on `,` or IME `Done`/Enter; space is a normal character. Pasted text is split on commas. Autocomplete suggestions come from `tags()` filtered with a case-insensitive contains across the whole tag (so `ui lib` matches `ui library`), most used first, max 8. Chips are removable; backspace on empty input pops the last chip into the text field. Commit applies `parse_tags` from Rust to normalize.

### D6. Share receiver

`ShareReceiverActivity` receives `ACTION_SEND` (`text/plain`, also `EXTRA_SUBJECT` as title). It extracts the first URL from the text, calls `normalize_url`, then `get(url)`:
- absent or tombstoned: fresh sheet, title from `EXTRA_SUBJECT`, `fetch_metadata` in the background for the description (and title when subject empty), TagEditor focused with IME up, Save calls `add` with `fetch: false` and the already-fetched fields (so Save never waits on the network).
- present: "Already saved on <date>" sheet, TagEditor prefilled, button "Update tags" calls `update(add_tags)`; "Edit" opens the full form in the main activity.
- offline (no validated network): skip fetch, show the cloud-slash note, save locally.
After Save: `Toast.makeText(context, "Saved to wobook · <tags>", LENGTH_SHORT)` then `finish()`. If the toast cannot be shown (API returns null or notifications disabled for toasts), show the in-sheet "Saved" state for 600 ms before finishing. "More" opens `form?url=` in the main activity with the current fields passed as arguments.

### D7. Pairing UI

- Scan: CameraX preview + ImageAnalysis with zxing-cpp (copy session-tap `ScanScreen.kt`), torch toggle, "Paste code instead" opens a text field; both paths call `domain.QrPayload.validate` then `join_pairing`. Camera permission denied state shows the design's frame with "Open settings" and the paste fallback.
- Show my QR: `start_pairing_offer()`, render with zxing-cpp encoder onto the light tile (modules in crust), countdown ring from `expires_at_ms`, "Copy as text" copies the JSON payload. Expiry navigates to the expired state; "Scan again"/"Show again" restarts.
- Confirm: `ConfirmRequired` event navigates to `pairing/confirm/{id}` showing peer name, platform icon, 4×8 hex fingerprint groups, Trust/Reject. Both devices confirm (symmetric, as in wobook-sync).
- Result states map 1:1 to `PairingEvent`: Completed -> Devices with snackbar "Paired with <name>", Expired, Rejected, Unreachable (lists tried endpoints), Failed.
- Desktop-initiated flow: desktop prints the QR via `wobook pair`; the phone scans. Android-to-Android: one shows, the other scans or pastes.

### D8. Sync lifecycle

Foreground: `ForegroundSyncLifecycle` acquires `WifiManager.MulticastLock` and calls `set_foreground(true)` on `ON_START`; `wobook-sync` then starts mDNS, the QUIC listener and reconnects known endpoints (LAN and Tailscale). `ON_STOP` releases and calls `set_foreground(false)`. Background: `BackgroundSyncWorker` as in D3, registered with `ExistingPeriodicWorkPolicy.UPDATE` when the setting is on, cancelled when off; a `BootReceiver` is not needed because WorkManager persists. Sync status UI reads `sync_status()` and the listener; Home shows only the 2 dp indeterminate line while `Syncing`; Devices footer and Settings show the textual state.

### D9. Secure key store

`KeystoreSecureStore` implements the UniFFI `SecureKeyStore` callback interface: a non-exportable AES-256-GCM key in `AndroidKeyStore` (alias `wobook-wrap`, no user authentication required), secrets wrapped with a random 12-byte IV and AAD `wobook:<kind>:v1`, ciphertext stored at `filesDir/secure/<kind>.bin`. `load` returns `None` when the file is missing; a decryption failure is surfaced as `SecureStoreError::Corrupt` so Rust can treat the identity as lost (the user re-pairs). Linux `wobook-sync` keeps its file-based store; both implement the same trait on the Rust side (`wobook_sync::SecureKeyStore` with `load/store/remove` by kind), and `wobook-ffi` adapts the callback interface to it.

### D10. Testing

Maestro flows in `android/maestro/*.yaml`, run by `android/scripts/e2e.sh` inside `nix develop .#android-emulator`:
1. Start emulator (`android/scripts/emulator.sh`, headless, x86_64 API 36), build and install debug APK.
2. Start a desktop fixture: `wobookd` with a temp data dir and `--listen 0.0.0.0:<port>` (hosts reachable from the emulator as `10.0.2.2`), seeded with `wobook add`.
3. Flows: `onboarding.yaml` (name, start fresh, empty Home), `share-add.yaml` (`adb shell am start -a android.intent.action.SEND -t text/plain --es android.intent.extra.TEXT <url>`, type tags with a space inside, Save, verify in Home), `search-open.yaml` (type abbreviation, assert first result, tap Open, assert browser intent via `adb shell dumpsys activity` or Maestro `assertVisible` on Chrome), `edit-tags.yaml`, `delete-undo.yaml`, `pair-paste.yaml` (run `wobook pair --json` on the fixture, paste payload, confirm fingerprint on both sides with `wobook pair --yes`), `devices.yaml`, `sync-converge.yaml` (`wobook add` on the fixture, assert it appears on the phone within 30 s; add on the phone, assert `wobook show` on the fixture).
4. Pairing payload endpoints: the fixture advertises `10.0.2.2:<port>` via `--advertise` so the emulator can reach it.
Unit tests (JVM, no Robolectric): `QrPayloadTest`, `TagEditorRulesTest`, `UrlDisplayTest`. Rust: `wobook-ffi` has a smoke test opening an app on a temp dir with in-memory key store and listener, exercising add/search/pairing offer.

## Risks / Trade-offs

- [UniFFI async + callback interfaces are newer APIs] → pin `uniffi` to one 0.29.x, keep callback interfaces synchronous (they return quickly), run all Rust async on the internal Tokio runtime and expose `suspend` functions.
- [mDNS on Android is unreliable without the multicast lock, and useless in the background] → lock held only in the foreground; background worker relies on remembered endpoints (LAN + Tailscale) rather than discovery.
- [WorkManager 15 min minimum and Doze] → acceptable; the design states minutes of lag are fine. "Sync now" exists for impatience.
- [QUIC listener on the phone behind carrier NAT] → the phone always dials out to known endpoints; inbound only matters on LAN/tailnet, where it works.
- [Share receiver must finish fast even when fetch is slow] → fetch runs concurrently; Save never awaits it; whatever arrived is saved, the rest can be synced from a desktop hook later.
- [Design fidelity drift] → one frame = one `testTag`, screenshots of each frame compared manually during review (design README forbids automated rendering of the prototype).
- [Keystore key lost on reinstall] → identity is lost; app shows onboarding as a new device; peers must revoke the old one. Documented in Settings › About.

## Migration Plan

1. Build debug APK in `nix develop .#android`, sideload with `adb install`.
2. On a desktop run `wobook pair`, scan with the phone, confirm on both sides.
3. Library syncs; phone is now a peer. Removing the app deletes its local copy only.

## Open Questions

- Whether `p2p-sync` already exposes the `SecureKeyStore` trait by kind; if it exposes fi's richer trait instead, `wobook-ffi` adapts it (the spec delta here is conditional).
- Phosphor icons: Compose library `com.adamglin:phosphor-icon` (session-tap) versus bundling SVGs; default to the library.
