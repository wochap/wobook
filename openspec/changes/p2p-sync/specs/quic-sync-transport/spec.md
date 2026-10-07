## ADDED Requirements

### Requirement: QUIC transport with pinned keys
The daemon SHALL listen on UDP in the range 47390-47399 (first free port, persisted) using quinn with rustls TLS 1.3, ALPN `wobook-sync/1`, a self-signed Ed25519 certificate (SAN `wobook.invalid`, CN = device id), and SHALL authenticate peers by extracting the certificate public key and requiring a `trusted` row, in both directions.

#### Scenario: Trusted peers connect
- **WHEN** A and B trust each other and A dials B's endpoint
- **THEN** the handshake succeeds and `sync.status` on both shows the peer as connected

#### Scenario: Revoked peer rejected
- **WHEN** B is revoked on A and dials A
- **THEN** A's handshake fails and B's `sync.status` shows A with `last_error = device_revoked`

#### Scenario: Port persisted
- **WHEN** the daemon first binds port 47391 because 47390 is busy, then restarts with 47390 free
- **THEN** it binds 47391 again

### Requirement: Transport implements automerge_repo NetworkTransport
The transport SHALL deliver `PeerConnected`, `Message` and `PeerDisconnected` events with the authenticated device id as `PeerId`, preserve frame order and completeness per peer, and keep at most one connection per peer, newer replacing older.

#### Scenario: Convergence after offline edits
- **WHEN** A and B are paired, both edit different and overlapping bookmarks while disconnected, and reconnect
- **THEN** within 10 seconds both daemons report equal heads and `list` output is identical

#### Scenario: Duplicate connection
- **WHEN** B opens a second connection to A while one exists
- **THEN** A keeps exactly one connection to B and no frames are lost

### Requirement: Admission policy
Inbound connections SHALL be admitted from any unicast source address, including tailnet addresses, and at most 8 connections SHALL be open at once.

#### Scenario: Tailnet source
- **WHEN** a trusted peer connects from `100.84.12.7`
- **THEN** the connection is accepted

#### Scenario: Connection cap
- **WHEN** 8 connections are open and a ninth trusted peer connects
- **THEN** the ninth is refused and retried later

### Requirement: Endpoint racing and reconnect
Dialing a peer SHALL start with its most recently successful endpoint and launch the remaining endpoints 300 milliseconds later in parallel, keep the first successful handshake, record success or failure per endpoint, and retry disconnected peers with jittered backoff from 1 to 30 seconds while the daemon runs.

#### Scenario: Stale LAN address
- **WHEN** B's remembered LAN address no longer answers but its tailnet address does
- **THEN** A connects over the tailnet address and marks the LAN address as failed

#### Scenario: Reconnect after restart
- **WHEN** B restarts
- **THEN** A reconnects within 30 seconds without user action

### Requirement: Control stream
After the handshake each side SHALL open a control stream carrying `hello` (name, platform, discovery epoch), discovery secret updates and sync nudges, separate from the Automerge frames.

#### Scenario: Sync now
- **WHEN** `sync.now` is called on A
- **THEN** A dials every trusted peer that is not connected and sends a nudge on every open control stream
