## ADDED Requirements

### Requirement: Pairing commands
The daemon SHALL implement `pair.start` (returns payload JSON, terminal QR text and expiry), `pair.join` (payload in, session id out, proceeds asynchronously), `pair.pending` (list of sessions with role, peer name, platform, fingerprint, stage, expiry), `pair.confirm` and `pair.reject` (by session id). Errors SHALL use codes `pair_window_closed`, `pair_rate_limited`, `pair_pending_missing` and `invalid_request`.

#### Scenario: Start then pending
- **WHEN** `pair.start` is called on A and `pair.join` with A's payload on B
- **THEN** within 5 seconds `pair.pending` on both returns one session with the same fingerprint

#### Scenario: Confirm unknown session
- **WHEN** `pair.confirm` is called with a session id that does not exist
- **THEN** the error code is `pair_pending_missing`

### Requirement: Device commands
The daemon SHALL implement `devices.list`, `devices.rename`, `devices.revoke` and `devices.add_endpoint`, resolving devices by id, and SHALL return `unknown_device` for ids not in the trust store and `device_revoked` when acting on a revoked device other than listing it.

#### Scenario: List after pairing
- **WHEN** A and B are paired
- **THEN** `devices.list` on A returns B with id, name, platform, `paired_at_ms`, endpoints and `revoked = false`

#### Scenario: Add endpoint validation
- **WHEN** `devices.add_endpoint` receives `not-an-address`
- **THEN** the error code is `invalid_request`

### Requirement: Sync and device-name commands
The daemon SHALL implement `sync.status`, `sync.now`, and `device.name` (get when `name` is absent, set when present, non-empty, at most 64 characters).

#### Scenario: Set name
- **WHEN** `device.name` is called with `name = "laptop"`
- **THEN** a following `device.name` returns `laptop` and `status.device.name` is `laptop`

#### Scenario: Empty name rejected
- **WHEN** `device.name` is called with an empty string
- **THEN** the error code is `invalid_request`

### Requirement: Status includes device and peers
`status` SHALL include `device: {id, name, port}` and `peers: {trusted, connected}` and, when applicable, `recovery`.

#### Scenario: Status after pairing
- **WHEN** A is paired with B and B is connected
- **THEN** A's `status.peers` is `{trusted: 1, connected: 1}`
