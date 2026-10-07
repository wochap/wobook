## ADDED Requirements

### Requirement: Scan screen
The Scan screen SHALL show a CameraX preview with a 260 dp framing square, a torch toggle, the hint "On a desktop run `wobook pair`. On a phone open Devices → Show my QR.", and a "Paste code instead" fallback. Decoded or pasted payloads SHALL be validated locally before `join_pairing` is called, and invalid payloads SHALL show an inline error without a network call.

#### Scenario: Paste valid payload
- **WHEN** the user pastes a valid pairing payload
- **THEN** the Connecting state shows "Connecting to <peer>… via <LAN|Tailscale> · <address>" with the 2 dp progress line

#### Scenario: Invalid payload
- **WHEN** the user pastes text that is not a pairing payload
- **THEN** an inline error is shown and no connection is attempted

#### Scenario: Camera permission denied
- **WHEN** camera permission is denied
- **THEN** the screen shows the camera-off explanation, an "Open settings" button and the paste fallback

### Requirement: Show my QR
The Show my QR screen SHALL call `start_pairing_offer`, render the payload as a QR with dark modules on a light tile (readable in dark theme), show the device name, a 120 second countdown ring and "Copy as text". On expiry it SHALL show the expired state with a restart action.

#### Scenario: Copy as text
- **WHEN** the user taps Copy as text
- **THEN** the clipboard holds the exact JSON payload

#### Scenario: Expiry
- **WHEN** 120 seconds pass without a pairing
- **THEN** the expired state is shown and the QR is no longer displayed

### Requirement: Fingerprint confirmation
When a `ConfirmRequired` event arrives the app SHALL show the peer name, platform icon and the fingerprint as four groups of eight hex characters in monospace, with Reject and Trust buttons and the explanation that the other device shows the same groups.

#### Scenario: Trust
- **WHEN** the user taps Trust and the peer also confirms
- **THEN** the app navigates to Devices, shows the new peer as syncing and a snackbar "Paired with <name>"

#### Scenario: Reject
- **WHEN** the user taps Reject
- **THEN** nothing is stored and the Devices screen is shown unchanged

### Requirement: Pairing result states
The app SHALL render dedicated states for Expired (with "Scan again"), Rejected by the other side, Unreachable (listing tried endpoints and the LAN/tailnet hint, with Retry and Cancel) and generic Failed, mapped from `PairingEvent`.

#### Scenario: Unreachable
- **WHEN** no endpoint from the payload answers
- **THEN** the Unreachable state lists the tried addresses and offers Retry
