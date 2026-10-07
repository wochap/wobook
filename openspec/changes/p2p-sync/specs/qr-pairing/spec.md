## ADDED Requirements

### Requirement: Pairing payload
`pair.start` SHALL produce a JSON payload `{"v":1,"name","id","ep":[...],"s","exp"}` where `id` is the offerer's device id, `ep` lists `ip:port` reachability hints (interface addresses excluding docker, veth, virbr, br- and link-local, plus tailnet addresses), `s` is a fresh base64url 32-byte secret and `exp` is 120 seconds from now. The payload SHALL be rendered as a terminal QR code and as text.

#### Scenario: Payload shape
- **WHEN** `pair.start` is called
- **THEN** the payload has `v = 1`, a 64-hex `id`, at least one `ep`, a 32-byte `s` and `exp` within 120 seconds

#### Scenario: Payload validation before connecting
- **WHEN** `pair.join` receives a payload with a malformed `id`, a non-32-byte secret, zero endpoints or an `exp` more than 30 seconds in the past
- **THEN** it fails with `invalid_request` and opens no connection

### Requirement: Pairing window
A pairing window SHALL last 120 seconds, accept exactly one successful proof, and be replaced by a newer `pair.start`. Outside a window the pairing ALPN SHALL reject connections.

#### Scenario: Second joiner refused
- **WHEN** a joiner completes the proof and a second joiner presents the same secret
- **THEN** the second receives `pair.error` with code `expired`

#### Scenario: Window expired
- **WHEN** a joiner connects 130 seconds after `pair.start`
- **THEN** it receives `pair.error` with code `expired`

### Requirement: Proof of possession
The joiner SHALL prove possession of the secret with `HMAC-SHA256(secret, "wobook-pair-v1" || offerer_pubkey || joiner_pubkey || nonce)` where both public keys are those authenticated by the QUIC/TLS handshake and `nonce` is 32 random bytes from the offerer.

#### Scenario: Valid proof
- **WHEN** the joiner computes the MAC with the secret from the payload
- **THEN** the offerer accepts and both sides enter the confirmation stage

#### Scenario: Wrong secret
- **WHEN** the joiner uses a different secret
- **THEN** the offerer replies `pair.error` with code `bad_proof` and the window stays open

#### Scenario: Known test vector
- **WHEN** the MAC is computed for the fixed secret, keys and nonce in the test vector
- **THEN** it equals the expected hex string

### Requirement: Fingerprint confirmation on both sides
After a valid proof, both devices SHALL expose a pending pairing with the peer's name, platform and fingerprint (first 16 bytes of `sha256(peer_pubkey)` as four groups of eight lowercase hex characters) and SHALL only store trust after a local confirmation and the peer's positive decision. Confirmation SHALL time out after 120 seconds.

#### Scenario: Both confirm
- **WHEN** both sides call `pair.confirm`
- **THEN** both insert the peer as trusted and the offerer sends provisioning

#### Scenario: One rejects
- **WHEN** either side calls `pair.reject`
- **THEN** the other receives `pair.error` with code `rejected` and neither stores trust

#### Scenario: Fingerprint format
- **WHEN** a pending pairing is listed
- **THEN** `fingerprint` matches `^[0-9a-f]{8}( [0-9a-f]{8}){3}$` and is identical on both devices

### Requirement: Provisioning
After mutual confirmation the offerer SHALL send the root document id, the list of trusted devices with their endpoints, the discovery group secret with epoch and its sync port. The joiner SHALL store all of it and open a sync connection.

#### Scenario: Joiner learns the whole mesh
- **WHEN** C pairs with A, and A already trusts B
- **THEN** C's `devices.list` contains both A and B with endpoints, and A announces C to B so B trusts C as well

### Requirement: Joining with existing local bookmarks
When the joiner's storage already holds its own root document, the joiner SHALL export its non-deleted bookmarks to `<data_dir>/pre-join-<timestamp>.jsonl`, move the old document to `quarantine/pre-join-<timestamp>/`, join the offerer's root, wait for the first full sync, then import the export with merge-by-URL semantics.

#### Scenario: Local bookmarks survive join
- **WHEN** B has 10 bookmarks of its own and joins A which has 20 bookmarks, 3 of them with URLs shared with B
- **THEN** after convergence both A and B list 27 bookmarks and the 3 shared ones have the union of tags

#### Scenario: Old document kept
- **WHEN** the join above completes
- **THEN** the previous document file exists under `quarantine/` and the JSONL export exists in the data directory

#### Scenario: Different mesh refused
- **WHEN** B is already joined to a root different from A's
- **THEN** pairing fails with `pair.error` code `protocol` and B's data is untouched

### Requirement: Rate limiting
The offerer SHALL lock out a joiner key after 3 failed proofs, a source address after 5 failed proofs, burn the window after 20 failures in total, and limit pairing connections to 10 per minute per source address.

#### Scenario: Lockout per key
- **WHEN** a joiner fails the proof 3 times
- **THEN** its fourth attempt receives `pair.error` code `rate_limited` without MAC verification
