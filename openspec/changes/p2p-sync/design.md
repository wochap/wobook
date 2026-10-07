## Context

`core-and-daemon` leaves `wobookd` driving `automerge_repo::Repo` with a `NullTransport`. `automerge_repo` already implements the Automerge sync protocol, per-peer sync state, inventory and announcements over any `NetworkTransport` that delivers authenticated peer ids and ordered complete frames. This change supplies that transport plus everything around it: who is trusted, how trust is established, how peers find each other.

Two reference implementations exist on this machine and are the source for nearly all code:

- fi, `/home/gean/Sandboxes/sandbox/fi/crates/app_core/src/`:
  - `identity.rs` (DeviceId, PublicDeviceKey, PrivateDeviceKey, DeviceIdentity, `SecureKeyStore` trait, `InMemorySecureKeyStore`; skip `LinuxSecretServiceKeyStore`).
  - `quinn_transport.rs` (`TlsIdentity::generate` with rcgen Ed25519 self-signed cert, `extract_public_key`, `TrustResolver` trait, pinned rustls verifiers, `QuinnTransport` implementing `automerge_repo::NetworkTransport`, `admits_inbound`, pairing peer admission).
  - `discovery.rs` (`DiscoveryGroupSecret`, service type derivation, `DiscoveryAdvertisement`, `DiscoveredEndpoint`, `TailnetProbe`, `is_tailnet_ip`), `discovery_control.rs` (secret rotation messages).
  - `routing.rs` (`NetworkEndpoint` failure/success backoff, `EndpointRegistry`), `endpoint_memory.rs` (manual endpoints).
  - `adapters.rs` lines 290-420 (`SqliteControlStore` tables `trusted_devices`, `peer_endpoints`, `discovery_group`, `discovery_rotation`, `recovery_attempts`).
  - `pairing.rs` `reduce_pairing` as the shape of a pure pairing state machine (the SAS content is not reused).
  - `docs/operations.md` for recovery semantics.
- session-tap, `/home/gean/Sandboxes/sandbox/session-tap/`:
  - `crates/sessiontap-hub/src/service.rs` (`PAIR_TTL`, `PAIR_LABEL`, `pair_mac`, `PairRateLimiter`, fingerprint formatting, `pair_device`), `tls.rs`, `endpoints.rs` (`endpoint_hints` skipping docker/veth/virbr/br-/link-local), `cli.rs` (`render_qr`).
  - `docs/hub.md` lines 341-590 for the QR payload and handshake prose.
  - `android/app/src/main/java/dev/sessiontap/android/net/Connection.kt` `raceEndpoints` (parallel racing, 300 ms head start).

Everything that was Flutter-, collection- or hub-specific is dropped.

## Goals / Non-Goals

**Goals:**
- Any two wobook devices pair with one QR scan or one pasted string and converge afterwards with no server.
- Works on LAN and over Tailscale. Off-LAN without Tailscale is not attempted.
- Trust is explicit: both humans compare a fingerprint; revoke is immediate.
- A device that already has bookmarks can join an existing mesh without losing them.
- The transport is a drop-in `NetworkTransport`; `wobook-core` and the document format stay untouched.
- Android (next change) reuses `wobook-sync` through UniFFI with only a `SecureKeyStore` implementation of its own.

**Non-Goals:**
- Relays, NAT traversal, STUN, DERP. Tailscale is the answer.
- Scopes or per-device permissions. All trusted devices are equal.
- Linux Secret Service for the key (LUKS covers disk at rest).
- Background sync policy on Android (android-app).
- Multiple documents or collections.

## Decisions

### D1. Crate layout

```
crates/wobook-sync/src/
  identity.rs        DeviceId, keys, DeviceIdentity, SecureKeyStore, FileKeyStore (0600 identity.key)
  control.rs         SqliteControlStore: trusted_devices, peer_endpoints, discovery_group, discovery_rotation, recovery_attempts, device_name
  pairing/
    payload.rs       QrPayload v1 encode/decode/validate
    reducer.rs       pure state machine for offerer and joiner (events in, actions out)
    proof.rs         pair_mac, fingerprint, rate limiter
    manager.rs       drives reducer over QUIC streams, exposes pending confirmations
  transport.rs       QuinnTransport (fi quinn_transport.rs, renamed constants, LAN-only rule removed)
  discovery.rs       mDNS advertise + browse (fi discovery.rs), service type from group secret
  endpoints.rs       endpoint memory + ranking + racing + interface hints (fi routing.rs + endpoint_memory.rs + session-tap endpoints.rs)
  rotation.rs        discovery secret rotation on revoke (fi discovery_control.rs)
  lib.rs             SyncEngine facade: open(data_dir, key_store, repo) -> handles for wobookd
```
Package name `wobook-sync`, lib `wobook_sync`. Depends on `automerge_repo` and `wobook-core` (for `protocol` DTOs only). `wobookd` depends on it; `wobook` CLI does not (talks to the daemon).

Alternative: keep everything in `wobookd`. Rejected because Android links the same crate through UniFFI.

### D2. Constants (fi values renamed)

| Item | Value |
| --- | --- |
| Sync ALPN | `wobook-sync/1` |
| Pairing ALPN | `wobook-pair/1` |
| Certificate SAN | `wobook.invalid`, CN = DeviceId hex |
| mDNS service type | `_wobook-<10 lowercase base32 chars>._udp.local.` derived as in fi `discovery.rs` from the group secret |
| UDP port range | 47390-47399, first free port taken, persisted in `control.sqlite` as `sync_port` |
| Pairing window | 120 s, one success |
| Pairing HMAC label | `wobook-pair-v1` |
| Frame magic | unchanged `FIRP` from `automerge_repo` |

### D3. Identity

Ed25519 keypair. `DeviceId = sha256(public_key)` (fi `DeviceId::from_public_key`), hex for display and SQL. `FileKeyStore` implements `SecureKeyStore`: `identity.key` holds the 32-byte seed, mode 0600, created on first start; the public key is also inserted into `control.sqlite.local_identity`. Android will supply a Keystore-backed implementation in its own change. `device_name` lives in `control.sqlite` (`device_name` singleton), defaults to the hostname, changeable at any time; peers learn the new name at the next connection in the `Hello` extension (see D6).

Alternative: Linux Secret Service as in fi. Rejected: adds a D-Bus dependency to a daemon that runs headless under systemd, the disk is LUKS-encrypted already.

### D4. Trust store (`control.sqlite`)

Copy the fi `SqliteControlStore` schema shape, keep only:

```sql
local_identity(singleton, device_id, public_key, created_at_ms)
device_name(singleton, name, updated_at_ms)
trusted_devices(device_id PK, public_key, name, platform TEXT CHECK IN ('linux','android'), paired_at_ms, last_seen_ms, last_sync_ms, trust_state CHECK IN ('trusted','revoked'))
peer_endpoints(device_id, address, kind CHECK IN ('lan','tailnet','manual'), last_success_ms, last_failure_ms, PRIMARY KEY(device_id,address))
discovery_group(singleton, epoch, secret BLOB 32)
discovery_rotation(singleton, previous_epoch, previous_secret, target_epoch, retain_until_ms, stage)
recovery_attempts(root PK, attempts)
sync_port(singleton, port)
```
STRICT tables, `synchronous=FULL`, opened with the session-tap `open_private_sqlite` helper already copied in `core-and-daemon`. `TrustResolver` for the transport reads `trusted_devices` where `trust_state='trusted'`; revoked keys are kept forever so they can never re-pair silently without a human confirming a new fingerprint.

Revoke: set `revoked`, close its connections, delete its endpoints, start a discovery secret rotation (D8) so the revoked device stops seeing the group's mDNS name.

### D5. QR payload

```json
{"v":1,"name":"gdesktop","id":"<hex sha256(public_key)>","ep":["192.168.1.40:47390","100.84.12.7:47390"],"s":"<base64url 32 bytes>","exp":1759800120}
```
`ep` is built with session-tap's `endpoint_hints` (all interface addresses on the chosen port, skipping docker/veth/virbr/br-/link-local, tailnet addresses appended) and is a reachability hint only; trust comes from `id` and `s`. Validation before any network call (session-tap `QrValidation.kt`): `v == 1`, `id` 64 hex, 1..=16 endpoints that parse as `SocketAddr`, `s` decodes to 32 bytes, `exp` within 120 s of now (allow 30 s clock skew). `wobook pair` renders the QR with the `qrcode` crate in the terminal (unicode half-blocks) and prints the JSON on the next line for copy-paste.

### D6. Pairing handshake

Offerer = the device that called `pair.start` (shows QR). Joiner = the device that scanned or pasted (`pair.join`). Transport: a QUIC connection with ALPN `wobook-pair/1` to the first reachable `ep` (racing per D9). During the window the offerer's `QuinnTransport` admits unknown peers on the pairing ALPN only (fi `admit_pairing_peer`); the sync ALPN still rejects them.

Messages on one bidirectional stream, JSON lines, each capped at 64 KiB:

1. joiner → `{"type":"pair.begin","name":"pixel-8","platform":"android"}`
2. offerer → `{"type":"pair.nonce","nonce":"<base64url 32B>","name":"gdesktop","platform":"linux"}`
3. joiner → `{"type":"pair.complete","mac":"<hex>"}` with `mac = HMAC-SHA256(secret, "wobook-pair-v1" || offerer_pubkey || joiner_pubkey || nonce)`. Both public keys are the ones proven by the TLS handshake (`extract_public_key` from the peer certificate).
4. offerer verifies, marks the window consumed, then both sides enter `AwaitingConfirmation` and expose the pending pair through `pair.pending`: `{peer_name, peer_platform, fingerprint, expires_at}`. Fingerprint = first 16 bytes of `sha256(peer_pubkey)` as 4 groups of 8 lowercase hex. Both humans compare and answer `pair.confirm` or `pair.reject` (CLI prompts `Trust this device? [y/N]`). Confirmation timeout 120 s.
5. offerer → `{"type":"pair.decision","trusted":true|false}` and joiner → same. Both must be true.
6. offerer → `{"type":"pair.provision","root":"<DocumentId>","devices":[{id,name,platform,endpoints:[...]}],"group":{"epoch":n,"secret":"<base64url>"},"sync_port":47390}`.
7. joiner → `{"type":"pair.done"}`. Both insert the other into `trusted_devices` with endpoints, close the pairing stream, and open the sync ALPN connection.

Failure at any step sends `{"type":"pair.error","code":"bad_proof|expired|rejected|rate_limited|protocol"}` and closes. Rate limits copy session-tap `PairRateLimiter`: 3 failed proofs per joiner key, 5 per source IP (/64 for IPv6), 20 total failures burn the window; `pair.*` connections per address limited to 10 per minute.

The reducer (`pairing/reducer.rs`) is pure: `reduce(state, event) -> (state, Vec<action>)` for both roles, with events `Begin`, `Nonce`, `Complete`, `LocalDecision`, `RemoteDecision`, `Provision`, `Done`, `Timeout`, `Error` and actions `Send(msg)`, `ExposePending`, `PersistTrust`, `Join(root)`, `Fail(code)`. Unit tests cover every transition; the manager only does IO.

Symmetry: a phone can be offerer (shows QR) or joiner (scans); a desktop is offerer (`wobook pair`) or joiner (`wobook pair --join '<json>'`). Nothing in the protocol depends on platform.

### D7. Joining with existing data

The joiner's `Repo` state decides:

- Fresh storage (no bootstrap record, which is the normal case right after install) → `repo.join_existing(root)`; `automerge_repo` fetches the document from the offerer.
- Storage already initialized with its own root (user ran wobook standalone first) → before `join_existing`: export every non-deleted local bookmark to `<data_dir>/pre-join-<timestamp>.jsonl`, quarantine the old document directory under `quarantine/pre-join-<timestamp>/` (never delete), reset the bootstrap record, `join_existing(root)`, wait for the first full sync, then import the JSONL with the `merge:true` path from `core-and-daemon`. The CLI prints the counts. Tombstones from the old document are not carried over (they only affected the local copy).
- Storage already joined to the same root → no-op beyond adding the peer.
- Storage joined to a different root → `pair.error` `protocol` with message "this device belongs to another wobook mesh; reset it first" (`wobook reset --data` is out of scope; the user deletes the data dir).

Alternative: merge the two Automerge documents natively. Rejected: they have unrelated histories and Automerge cannot merge documents with different actor roots meaningfully; URL-keyed import gives the same result.

### D8. Discovery and secret rotation

fi `discovery.rs` as-is with renamed prefix. Each device advertises `_wobook-<tag>._udp.local.` with TXT `v=1`, `id=<device_id>`, `port`, `epoch`. Browsing only refreshes `peer_endpoints` for devices already in `trusted_devices`; an unknown `id` is ignored, so discovery never grants trust. Multicast is LAN-only by nature; tailnet addresses come from `TailnetProbe` (fi) reading `100.64.0.0/10` interface addresses and are stored as `kind='tailnet'`.

Rotation on revoke (fi `discovery_control.rs`): generate a new secret and epoch, keep the previous for 24 h (`retain_until_ms`), push `DiscoverySecretUpdate` to every connected trusted peer over the sync connection's control stream, peers ack and switch. The revoked device still knows the old name for at most 24 h and is rejected at TLS anyway.

### D9. Transport, admission and endpoint racing

`QuinnTransport` from fi with: `SYNC_ALPN` renamed, `admits_inbound` returns true for all unicast addresses (drop the LAN-only policy; Tailscale peers arrive from `100.64.0.0/10`), `max_connections = 8`, idle timeout 60 s with keepalive 15 s, one connection per peer (newer wins, fi behaviour). Authentication: rustls verifiers pin the peer certificate's Ed25519 key, then `TrustResolver` must return `trusted`; unknown or revoked keys fail the handshake before any application byte.

Connecting to a peer: take its endpoints ordered by `last_success_ms` desc; start the most recent immediately and the rest 300 ms later in parallel (session-tap `raceEndpoints`); first handshake wins, others are aborted; record success/failure per endpoint (fi `NetworkEndpoint::record_*`, backoff 1 s to 30 s, jittered). Reconnect loop per trusted peer while the daemon runs; a `PeerDisconnected` event schedules the next attempt. Observed source addresses of inbound connections are added to `peer_endpoints` (fi `ObservedPeerAddress`).

The `Hello` frame of `automerge_repo` is untouched; the device name and platform travel in a `wobook-sync` control stream opened right after the handshake (`{"type":"hello","name","platform","epoch"}`), which also carries rotation messages and `sync.now` nudges.

### D10. Daemon integration and API

`wobookd` startup order: open store (core) → open `control.sqlite` → load or create identity → bind QUIC on the persisted or first free port → `Repo::open` with the real transport → start discovery → start reconnect loops. Shutdown reverses it.

New socket commands (same envelope and error codes as D9 of core-and-daemon, new codes `pair_window_closed`, `pair_rate_limited`, `pair_pending_missing`, `unknown_device`, `device_revoked`):

```
{"type":"pair.start"}                       -> {payload:{...QR json...}, qr_text:"<unicode QR>", expires_at}
{"type":"pair.join","payload":{...}}        -> {session:"<id>"}   (async; progress via pair.pending)
{"type":"pair.pending"}                     -> [{session, role:"offerer|joiner", peer_name, peer_platform, fingerprint, expires_at, stage}]
{"type":"pair.confirm","session":"..."}     {"type":"pair.reject","session":"..."}
{"type":"devices.list"}                     -> [{id, name, platform, paired_at_ms, last_seen_ms, last_sync_ms, reachability:"lan|tailnet|unreachable", endpoints:[{address,kind,last_success_ms}], revoked}]
{"type":"devices.rename","id","name"}       {"type":"devices.revoke","id"}       {"type":"devices.add_endpoint","id","address"}
{"type":"sync.status"}                      -> {device:{id,name,port}, peers:[{id,name,reachability,connected,in_progress,last_sync_ms,heads_equal}], discovery:{epoch,service_type}}
{"type":"sync.now"}                         -> connects to every trusted peer now and nudges sync
{"type":"device.name"}  {"type":"device.name","name":"..."}
```
`status` adds `device: {id, name, port}` and `peers: {trusted, connected}`.

The daemon subscribes to `repo.subscribe_peer_sync` and `DocHandle::subscribe()`; on a `DocumentEvent` with `origin: Remote(peer)` it reconciles the read model, diffs before/after records and emits `post-add`/`post-update`/`post-delete` hooks with `origin: "remote:<peer name>"`, then one `post-sync` with `{peer, changed: n}`.

### D11. CLI

```
wobook pair [--name <device-name>]          prints QR + JSON, then polls pair.pending, prompts "Trust this device? [y/N]", prints result
wobook pair --join '<json>'                 or reads JSON from stdin when the argument is "-"; same confirmation prompt
wobook devices list [--json]                table: name, platform, reachability dot, last synced, endpoints
wobook devices rename <id|name> <new-name>
wobook devices revoke <id|name>             asks "Revoke <name>? This cannot be undone. [y/N]"
wobook devices add-endpoint <id|name> <ip:port>
wobook sync status [--json] | wobook sync now
wobook device name [<new-name>]
```
`--yes` skips prompts for tests. Names resolve case-insensitively and must be unique among trusted devices, otherwise the id prefix is required.

### D12. Recovery (fi docs/operations.md, applied to one document)

| State on start | Action |
| --- | --- |
| Bootstrap record says joined, document file missing | enter Joining, fetch from any reachable peer, read model empty until then |
| Document file unreadable | move to `quarantine/<timestamp>/`, increment `recovery_attempts`, resync from a peer; after 3 attempts stop and surface `status.recovery = "needs_attention"` |
| `control.sqlite` unreadable | fatal, exit non-zero with the path |
| `identity.key` missing but `control.sqlite` has an identity | fatal with instructions (peers pinned the old key; re-pair after deleting the data dir) |
Nothing is ever auto-deleted.

### D13. Security summary

- Pre-authentication exposure: the QUIC endpoint accepts a TLS ClientHello from anyone on UDP 47390-47399. Without a trusted key the handshake fails; during a pairing window the pairing ALPN accepts a connection but reveals only a random nonce and the local name, and the proof requires the 32-byte QR secret.
- The QR string is a bearer secret for 120 s; the terminal output and the phone screen are the only places it appears; it is never logged.
- Fingerprint confirmation on both sides defeats an on-path attacker who could relay the proof.
- Firewall: allow UDP 47390-47399 inbound on LAN and `tailscale0`; nothing else listens. mDNS uses the standard 5353 multicast group.
- Revocation is immediate for connections and permanent for the key.

### D14. Testing

e2e (`crates/wobookd/tests/sync_e2e.rs`): two or three daemons on `127.0.0.1` with distinct data dirs, ports forced through `WOBOOK_SYNC_PORT`, discovery disabled through `WOBOOK_DISCOVERY=off`. Flows: pair via `wobook pair --json` output piped to `wobook pair --join -` with `--yes`; add on A appears on B; concurrent offline edits converge after reconnect; revoke on A closes B and B cannot reconnect (`device_revoked` in B's `sync status`); endpoint memory survives daemon restart; join with pre-existing local bookmarks merges them and quarantines the old document; wrong secret → `bad_proof`; expired payload → `expired`; `post-sync` and `remote:<name>` hooks fire on B. Unit tests: pairing reducer transitions, HMAC vector, fingerprint formatting, QR validation, endpoint ranking. mDNS browse/advertise checked manually on two real hosts (task marked manual).

## Risks / Trade-offs

- [automerge_repo join flow assumes fresh storage] → D7 export/quarantine/import path; e2e covers it.
- [Tailscale addresses change or the daemon binds before tailscale0 exists] → endpoints are hints, re-learned from inbound connections and `hello`; `sync.now` forces retries; the daemon rebinds on `0.0.0.0` so interface order does not matter.
- [8-connection cap] → three desktops plus a phone is far below it; configurable later.
- [Clock skew breaks `exp` validation] → 30 s tolerance, and the offerer also enforces its own 120 s window, so the payload `exp` is only a client-side hint.
- [Both humans must confirm, awkward for desktop-desktop over SSH] → `wobook pair --yes` exists for scripted setups; documented as weaker.
- [Secret rotation message lost while a peer is offline] → rotation is also sent in `hello` on every new connection, and the old secret stays valid 24 h.
- [Copying 3-5k lines from fi at low effort] → design lists exact source files and the rename table; tasks copy file by file with compile checkpoints.

## Migration Plan

1. Build, restart `wobookd` on the first desktop; it creates `identity.key` and `control.sqlite`, name defaults to hostname.
2. On the second desktop run `wobook pair --join -` with the JSON printed by `wobook pair` on the first; confirm fingerprints on both.
3. Open UDP 47390-47399 in the NixOS firewall for LAN and tailscale interfaces (manual until the nix-module change).
4. Rollback: stop the daemon; the document directory from `core-and-daemon` is unchanged and still readable by the previous binary.

## Open Questions

- None blocking. Whether the phone should also be reachable as an inbound peer (listening) or only dial out is decided in `android-app`; the transport supports both.
