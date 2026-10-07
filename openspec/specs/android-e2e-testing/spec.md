# android-e2e-testing Specification

## Purpose
Maestro flows, emulator shell, desktop peer fixture, unit tests limited to pure domain code.

## Requirements

### Requirement: Emulator and tooling shells
The flake SHALL provide `android` (SDK 36, build-tools 36.0.0, NDK, JDK 21, cargo-ndk) and `android-emulator` (adds x86_64 `google_apis` system image, emulator, Maestro, adb and the Rust toolchain) dev shells without removing the default Rust shell.

#### Scenario: Emulator boots
- **WHEN** `android/scripts/emulator.sh` runs in `nix develop .#android-emulator`
- **THEN** an API 36 x86_64 emulator boots headless and `adb devices` lists it

### Requirement: Desktop peer fixture
`android/scripts/e2e.sh` SHALL build and start a desktop `wobookd` on a temporary data directory listening on all interfaces and advertising `10.0.2.2:<port>` so the emulator can reach it, seed it with `wobook add`, and stop it after the flows.

#### Scenario: Fixture reachable
- **WHEN** the fixture runs and the emulator pairs by pasting the fixture's payload
- **THEN** the pairing completes through `10.0.2.2`

### Requirement: Maestro flows
Flows under `android/maestro/` SHALL cover: onboarding (name, start fresh), share-add via `adb shell am start -a android.intent.action.SEND` with a multi-word tag, search and open, edit tags, delete and undo, pairing by paste with fingerprint confirmation on both sides, devices list, and sync convergence in both directions (desktop add appears on the phone within 30 seconds, phone add appears in `wobook show` on the fixture).

#### Scenario: Full run
- **WHEN** `android/scripts/e2e.sh` runs
- **THEN** every flow passes and the script exits 0

#### Scenario: Sync convergence
- **WHEN** `wobook add https://conv.example/` runs on the fixture after pairing
- **THEN** the Home list on the emulator shows `conv.example` within 30 seconds

### Requirement: Unit tests limited to pure domain
JVM unit tests SHALL exist only for QR payload validation, TagEditor commit rules and URL display formatting. UI and data behaviour SHALL be covered by Maestro flows, not unit tests.

#### Scenario: Unit suite
- **WHEN** `./gradlew testDebugUnitTest` runs
- **THEN** the domain tests pass and no Robolectric dependency is required
