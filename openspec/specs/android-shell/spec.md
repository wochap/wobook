# android-shell Specification

## Purpose
app skeleton, theme tokens mapped to Compose, navigation, onboarding, settings, large-screen and dynamic-type rules.

## Requirements

### Requirement: Project configuration
The Android app SHALL live in `android/`, use Kotlin 2.x with Jetpack Compose Material 3, package `dev.wochap.wobook`, minSdk 31, compileSdk and targetSdk 36, JDK 21, a Gradle version catalog, and SHALL build inside `nix develop .#android`.

#### Scenario: Build from the Nix shell
- **WHEN** `./gradlew assembleDebug` runs in `nix develop .#android`
- **THEN** it produces `app/build/outputs/apk/debug/app-debug.apk`

### Requirement: Theme tokens from the design bundle
The app theme SHALL map the Catppuccin Mocha (dark, default) and Latte (light) tokens of `design/project/wobook-tokens.css` to the Material 3 `colorScheme`, typography and shapes exactly as `design/project/readme.md` specifies, including a `WobookColors` composition local for success, tailscale and warning colors. The theme SHALL follow the system dark/light setting. Inter SHALL be the UI font and JetBrains Mono the font for URLs and fingerprints. Icons SHALL be Phosphor Regular.

#### Scenario: Dark by default
- **WHEN** the system is in dark mode
- **THEN** the background is `#1e1e2e` and the primary color is `#89b4fa`

#### Scenario: Light follows system
- **WHEN** the system is in light mode
- **THEN** the background is `#eff1f5` and the primary color is `#1e66f5`

### Requirement: Navigation
A single activity SHALL host Compose Navigation routes for onboarding, home, detail, form, pairing (scan, show, confirm, result), devices and settings, with predictive back and edge-to-edge enabled. Each screen state from the design SHALL expose a `testTag` equal to the design frame id (for example `home-results`, `pair-confirm`).

#### Scenario: Deep navigation back
- **WHEN** the user navigates Home → Detail → Form and presses back twice
- **THEN** the user is on Home with the previous search state intact

### Requirement: Onboarding
On first launch the app SHALL show two screens: name this device (prefilled with the Android device model, Continue), then a choice between "Pair with an existing device" (goes to Scan) and "Start fresh" (goes to the empty Home). Onboarding SHALL not show again once completed. When onboarding ends through pairing (completed, or closed from a pairing result or a rejected confirmation), the app SHALL replace the whole back stack so that Home is its root and Devices is on top; onboarding and pairing screens SHALL NOT be reachable with Back afterwards.

#### Scenario: Start fresh
- **WHEN** the user enters a name and taps Start fresh
- **THEN** Home shows the empty state and `this_device().name` equals the entered name

#### Scenario: Pair during onboarding, then Back
- **WHEN** on a fresh install the user names the device, chooses "Pair with an existing device", confirms the fingerprint, lands on Devices and presses Back
- **THEN** Home is shown, and pressing Back again leaves the app instead of showing Scan or an onboarding screen

#### Scenario: Failed pairing during onboarding, then Close
- **WHEN** a pairing started from onboarding ends on a result state and the user taps Close
- **THEN** Devices is shown with Home directly beneath it on the back stack

### Requirement: Settings
Settings SHALL offer: device name (editable), link to Devices, background sync toggle with "Sync now" and last-sync line, import (JSONL, Netscape HTML, buku database) and export (JSONL, Netscape HTML) through the system document picker, library size line, tap behaviour (Shows detail | Opens in browser), auto-fetch toggle, and an About section with version and licenses.

#### Scenario: Tap behaviour applies
- **WHEN** tap behaviour is set to "Opens in browser" and a result row is tapped
- **THEN** the URL opens in the default browser and Detail is not shown

#### Scenario: Export via picker
- **WHEN** the user chooses Export → JSONL and picks a destination
- **THEN** the file is written through the content resolver and contains one JSON record per bookmark

### Requirement: Large screens and dynamic type
Content SHALL be single-column, clamped to 640 dp and centered on wide screens. All text SHALL use `sp`, result rows SHALL use min-height 64 dp, titles and URLs SHALL stay single-line with ellipsis and the tag line SHALL clip with `+N` at font scale 1.3.

#### Scenario: Font scale 1.3
- **WHEN** the system font scale is 1.3 and Home shows results
- **THEN** no text overlaps, trailing buttons remain 48 dp and each row still shows title, URL and one tag line
