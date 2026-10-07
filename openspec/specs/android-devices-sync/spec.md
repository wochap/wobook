# android-devices-sync Specification

## Purpose
Devices screen, reachability, rename/revoke, foreground sync, background WorkManager sync, sync status surfaces.

## Requirements

### Requirement: Devices screen
Devices SHALL list this device first (name, "This device · Android", no menu), then peers with platform icon, reachability dot (LAN green, Tailscale lavender, unreachable outline), "synced <relative time>" line and an overflow menu with Rename and Revoke…, followed by Scan QR and Show my QR buttons and a footer with the sync status text.

#### Scenario: Peer reachability
- **WHEN** a peer is connected over a tailnet address
- **THEN** its row shows the lavender dot and "Tailscale · synced <time>"

#### Scenario: Rename
- **WHEN** the user renames a peer
- **THEN** the new name is shown and used in sync status lines

### Requirement: Revoke requires confirmation
Revoke SHALL be the only confirmation dialog in the app. It SHALL explain that the device stops syncing, must be paired again and keeps its local copy, and SHALL call `revoke_device` on confirm.

#### Scenario: Revoke
- **WHEN** the user confirms Revoke
- **THEN** the peer disappears from the list and its connections are closed

### Requirement: Foreground sync
While any activity is started the app SHALL hold a multicast lock and call `set_foreground(true)`; when the process goes to the background it SHALL call `set_foreground(false)` and release the lock.

#### Scenario: Return to foreground
- **WHEN** the app returns to the foreground on the same Wi-Fi as a desktop peer
- **THEN** sync status transitions to `Syncing` and then `UpToDate` without user action

### Requirement: Background periodic sync
When the setting is enabled the app SHALL schedule a WorkManager periodic worker every 15 minutes on unmetered networks that opens the app core, enables networking, calls `sync_now`, waits for `UpToDate` or 60 seconds, then disables networking. Disabling the setting SHALL cancel the worker. No persistent foreground service SHALL be used.

#### Scenario: Toggle off cancels
- **WHEN** the user turns background sync off
- **THEN** no periodic work remains enqueued

#### Scenario: Worker converges
- **WHEN** the worker runs while a desktop peer is reachable
- **THEN** bookmarks added on the desktop since the last sync are present when the app is next opened

### Requirement: Sync status surfaces
Sync status SHALL appear as the 2 dp line on Home while syncing, as the footer text on Devices ("Up to date", "Syncing with <device>", "No device reachable — changes saved locally") and as the last-sync line in Settings. Offline with local data SHALL show no banner on Home.

#### Scenario: No peer reachable
- **WHEN** no peer is reachable
- **THEN** Devices footer shows the cloud-slash message and Home shows nothing extra
