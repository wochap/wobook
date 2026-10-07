## ADDED Requirements

### Requirement: Each device has an Ed25519 identity
The daemon SHALL create an Ed25519 keypair on first start, store the 32-byte seed in `<data_dir>/identity.key` with mode 0600, record the public key in `control.sqlite`, and derive `DeviceId = sha256(public_key)` rendered as 64 lowercase hex characters.

#### Scenario: First start creates identity
- **WHEN** `wobookd` starts on a data directory without `identity.key`
- **THEN** `identity.key` exists with mode 0600 and `status` reports a 64-hex `device.id`

#### Scenario: Restart keeps identity
- **WHEN** the daemon restarts
- **THEN** `device.id` is unchanged

#### Scenario: Key file missing but identity recorded
- **WHEN** `identity.key` is deleted while `control.sqlite` still holds a public key
- **THEN** the daemon exits non-zero with a message explaining that peers pinned the old key and the data directory must be reset before re-pairing

### Requirement: Key storage is behind a SecureKeyStore trait
Identity loading and creation SHALL go through a `SecureKeyStore` trait so the file-backed store on Linux can be replaced by a platform store on Android without changing the daemon or pairing code.

#### Scenario: In-memory store in tests
- **WHEN** the sync engine is opened with an in-memory `SecureKeyStore`
- **THEN** identity, pairing and transport work without touching `identity.key`

### Requirement: Device name
Each device SHALL have a human-readable name stored in `control.sqlite`, defaulting to the hostname, changeable through the daemon API, and sent to peers at connection time.

#### Scenario: Default name
- **WHEN** the daemon starts for the first time on host `gdesktop`
- **THEN** `device.name` returns `gdesktop`

#### Scenario: Rename propagates
- **WHEN** device A renames itself and reconnects to trusted peer B
- **THEN** `devices.list` on B shows the new name for A
