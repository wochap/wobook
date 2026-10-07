## ADDED Requirements

### Requirement: Trusted devices are pinned public keys
`control.sqlite` SHALL hold a `trusted_devices` table with device id, public key, name, platform (`linux` or `android`), `paired_at_ms`, `last_seen_ms`, `last_sync_ms` and `trust_state` (`trusted` or `revoked`). Only `trusted` rows SHALL authenticate a peer.

#### Scenario: Unknown key rejected
- **WHEN** a device whose public key is not in `trusted_devices` attempts a sync connection
- **THEN** the TLS handshake fails and no application data is exchanged

#### Scenario: Pairing inserts a trusted row
- **WHEN** a pairing completes with both confirmations
- **THEN** each side has the other in `trusted_devices` with `trust_state = trusted` and the peer's name and platform

### Requirement: Revocation is permanent and immediate
Revoking a device SHALL set its `trust_state` to `revoked`, close its open connections, delete its endpoints and keep the row so the key can never be trusted again without a new pairing.

#### Scenario: Revoke closes and blocks
- **WHEN** device A revokes device B while B is connected
- **THEN** B's connection is closed within 5 seconds and B's reconnection attempts fail authentication

#### Scenario: Revoked row survives re-pair attempt
- **WHEN** a revoked device pairs again with a valid QR and both humans confirm
- **THEN** the row is updated to `trusted` only through that pairing, never by reconnecting

### Requirement: Endpoint memory
`peer_endpoints` SHALL store per trusted device each known address with `kind` (`lan`, `tailnet`, `manual`), `last_success_ms` and `last_failure_ms`, learned from pairing provisioning, mDNS, observed inbound source addresses and manual entry, and SHALL survive daemon restarts.

#### Scenario: Learned address persists
- **WHEN** A connects to B successfully at `192.168.1.40:47390`, then A restarts
- **THEN** `devices.list` on A still shows that endpoint with its `last_success_ms`

#### Scenario: Manual endpoint
- **WHEN** `devices.add_endpoint` is called with `100.84.12.7:47390`
- **THEN** the address is stored with `kind = manual` and used in the next connection attempt

### Requirement: Discovery group secret
`control.sqlite` SHALL hold the current discovery group secret and epoch, and during a rotation the previous secret with its retention deadline. The secret SHALL be generated on first start and replaced by provisioning when joining a mesh.

#### Scenario: Joiner adopts the group secret
- **WHEN** a device joins a mesh through pairing
- **THEN** its `discovery_group` row equals the offerer's epoch and secret

#### Scenario: Rotation on revoke
- **WHEN** a device is revoked
- **THEN** a new secret and epoch are stored, the previous secret is retained for 24 hours, and connected peers receive the update
