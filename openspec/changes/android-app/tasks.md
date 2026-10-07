## 1. FFI-friendly core and wobook-ffi crate

- [ ] 1.1 Audit `wobook-core` and `wobook-sync` public types used by the FFI; make them owned, `Send + Sync`, lifetime-free, with error enums; add `wobook_sync::SecureKeyStore` trait (`load/store/remove` by kind) if not present and implement it for the Linux file store.
- [ ] 1.2 Create `crates/wobook-ffi` (`cdylib` + `rlib`, `uniffi` 0.29 proc-macros, `uniffi::setup_scaffolding!()`, `src/bin/uniffi-bindgen.rs`) and add it to the workspace.
- [ ] 1.3 Implement `WobookApp::open` with `AppConfig`, `SecureKeyStore` and `AppListener` callback interfaces, internal Tokio runtime, repo + read model + sync wiring, `shutdown`.
- [ ] 1.4 Export bookmark operations (`add`, `update`, `rename`, `delete`, `restore`, `get`, `list`, `search` with `title_indices`/`url_indices`, `tags`, `fetch_metadata`, `import`, `export`, `normalize_url`, `parse_tags`) reusing core functions and daemon semantics.
- [ ] 1.5 Export pairing, devices and sync control (`start_pairing_offer`, `join_pairing`, `pending_confirmations`, `confirm_pairing`, `devices`, `this_device`, `rename_device`, `revoke_device`, `sync_status`, `sync_now`, `set_foreground`) and deliver `on_data_changed`, `on_sync_status`, `on_pairing_event` from background tasks.
- [ ] 1.6 Add the `wobook-ffi` smoke test (temp dir, in-memory key store and listener: open, add, search, pairing offer, reopen keeps data, corrupt key → `IdentityLost`).

## 2. Android project and build pipeline

- [ ] 2.1 Extend `flake.nix` with `android` and `android-emulator` dev shells copied from `/home/gean/Sandboxes/sandbox/session-tap/flake.nix` (SDK 36, build-tools 36.0.0, `includeNDK = true` with a pinned NDK, JDK 21, `cargo-ndk`; emulator shell adds x86_64 google_apis image, Maestro, Rust toolchain), keeping the default Rust shell.
- [ ] 2.2 Scaffold `android/` (settings.gradle.kts, `gradle/libs.versions.toml` modelled on session-tap's, app module `dev.wochap.wobook`, minSdk 31, compile/target 36, JDK 21, Compose BOM, Material 3, navigation, DataStore, WorkManager, CameraX, zxing-cpp, Phosphor, kotlinx-serialization; release signed with the debug key).
- [ ] 2.3 Add Gradle tasks that run `cargo ndk -t arm64-v8a -t x86_64` for `wobook-ffi` into `jniLibs` and `uniffi-bindgen generate --library … --language kotlin` into a generated source set before `preBuild`; `./gradlew assembleDebug` succeeds in `nix develop .#android`.
- [ ] 2.4 Implement `WobookApplication` (single `WobookApp`), `crypto/KeystoreSecureStore` (AES-GCM wrapping key `wobook-wrap`, AAD `wobook:<kind>:v1`, files under `filesDir/secure/`), `data/AppRepository` flows, `data/Settings` DataStore.

## 3. Theme, navigation, onboarding, settings

- [ ] 3.1 Read `design/project/wobook.dc.html`, `readme.md` and `wobook-tokens.css` in full; implement `ui/theme` (Mocha + Latte color schemes, `WobookColors`, Inter + JetBrains Mono typography, shapes) per the readme mapping.
- [ ] 3.2 Build shared components: `TagChip` (all states), `SearchField`, `HighlightedText`, `ResultRow`, `TagEditor` (comma/Enter commit, spaces allowed, suggestions across spaces, backspace pops chip), `FetchStateNote`, `ShimmerField`, `Fingerprint`, `QrTile`, `ScanFrame`, `ReachabilityDot`, `SyncStatusFooter`, `EmptyState`, `UrlField`, `DeviceRow`.
- [ ] 3.3 Implement single-activity navigation with routes, predictive back, edge-to-edge, 640 dp content clamp and `testTag`s equal to design frame ids.
- [ ] 3.4 Implement Onboarding (name device with model prefill, pair or start fresh) and persist completion.
- [ ] 3.5 Implement Settings (device name, Devices link, background sync toggle + Sync now + last sync, import/export via document picker, library size, tap behaviour segmented control, auto-fetch toggle, About).

## 4. Home, Detail, Form

- [ ] 4.1 Implement Home: focused search on launch, 60 ms debounced FFI search, highlighted matches, tag chip row with AND filtering persisted across navigation, footer count line, FAB hidden when IME visible, states empty / no results (Add prefilled with URL-like query) / results / idle / syncing.
- [ ] 4.2 Implement result row actions: tap per setting, Copy and Open 48 dp buttons, swipe-right copy, long-press action sheet (Open, Copy, Share, Edit, Delete) and delete snackbar with Undo.
- [ ] 4.3 Implement Detail (title, selectable mono URL, description, tags, saved date, last-changed device, Open/Copy/Share, Edit/Delete).
- [ ] 4.4 Implement Add/Edit form: fields, Save in app bar and above keyboard, auto-fetch with shimmer and fetched/failed/offline note plus Retry, existing-URL switch to Edit with banner, read-only `UrlField` with Change and rename note, rename on save, delete with undo.

## 5. Share receiver

- [ ] 5.1 Add `ShareReceiverActivity` (translucent theme, `ACTION_SEND text/plain`, URL extraction, `EXTRA_SUBJECT` title) hosting a `ModalBottomSheet` with the design's header, `FetchStateNote`, focused `TagEditor` and suggestions.
- [ ] 5.2 Implement fresh / already-saved ("Update tags") / fetch-failed / offline states, background fetch that never blocks Save, "More" handoff to the full form with carried fields.
- [ ] 5.3 Implement post-save plain-text toast `Saved to wobook · <tags>` with the 600 ms in-sheet fallback, then finish.

## 6. Pairing and devices

- [ ] 6.1 Implement Scan (CameraX + zxing-cpp adapted from session-tap `ScanScreen.kt`, torch, framing square, desktop/phone hint, paste fallback, camera-denied state) with `domain.QrPayload` validation before `join_pairing`.
- [ ] 6.2 Implement Show my QR (`start_pairing_offer`, QR on light tile, countdown ring, Copy as text, expired state).
- [ ] 6.3 Implement Confirm (fingerprint groups, Trust/Reject) and result states (Completed → Devices + snackbar, Expired, Rejected, Unreachable with tried endpoints, Failed) driven by `on_pairing_event`.
- [ ] 6.4 Implement Devices (this device row, peers with reachability dot and relative sync time, Rename, Revoke confirmation dialog, Scan/Show buttons, sync status footer).
- [ ] 6.5 Implement `ForegroundSyncLifecycle` (multicast lock + `set_foreground`) and `BackgroundSyncWorker` (15 min, unmetered, enabled by setting, cancel on disable) and wire sync status into Home line, Devices footer and Settings.

## 7. Tests

- [ ] 7.1 Add JVM unit tests: `QrPayloadTest`, `TagEditorRulesTest`, `UrlDisplayTest`.
- [ ] 7.2 Write `android/scripts/emulator.sh` (headless API 36 x86_64) and `android/scripts/e2e.sh` (build APK, start emulator, build and run `wobookd` fixture advertising `10.0.2.2:<port>`, seed with `wobook add`, run Maestro flows, stop everything).
- [ ] 7.3 Write Maestro flows: `onboarding.yaml`, `share-add.yaml` (am start SEND, multi-word tag), `search-open.yaml`, `edit-tags.yaml`, `delete-undo.yaml`, `pair-paste.yaml` (fixture `wobook pair --json`, paste, confirm both sides), `devices.yaml`, `sync-converge.yaml` (both directions, 30 s budget).
- [ ] 7.4 Run `cargo test -p wobook-ffi`, `./gradlew testDebugUnitTest assembleDebug` in `nix develop .#android`, and `android/scripts/e2e.sh` in `nix develop .#android-emulator`; fix failures. Manually compare each screen against its design frame in both themes.
