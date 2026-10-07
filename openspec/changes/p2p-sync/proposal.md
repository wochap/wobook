## Why

After `core-and-daemon` one Linux machine runs wobook, but the user has three desktops and a phone. Syncthing is what corrupted buku's database, so wobook needs its own device-to-device replication: no hub, no cloud, every device a peer. The Automerge document already merges; this change gives it a network, a trust model and a pairing flow that a phone can later drive with a QR code.

## What Changes

- New crate `crates/wobook-sync`: Ed25519 device identity, SQLite trust store (`control.sqlite`), QR-based pairing over QUIC with an HMAC proof and fingerprint confirmation, quinn QUIC transport implementing `automerge_repo::NetworkTransport`, mDNS address discovery, persisted endpoint memory with endpoint racing and reconnect backoff.
- `wobookd` replaces its `NullTransport` with the QUIC transport, opens a UDP port in 47390-47399, runs discovery, drives pairing sessions and exposes pairing, device and sync state over the socket API.
- Daemon API additions: `pair.start`, `pair.join`, `pair.pending`, `pair.confirm`, `pair.reject`, `devices.list`, `devices.rename`, `devices.revoke`, `devices.add_endpoint`, `sync.status`, `sync.now`, `device.name`; `status` gains device id, name and peer summary.
- CLI additions: `wobook pair [--join <json>]`, `wobook devices list|rename|revoke|add-endpoint`, `wobook sync status|now`, `wobook device name [<name>]`.
- Hooks: `post-sync` fires after a peer session changes heads; remote-originated `post-add`/`post-update`/`post-delete` carry `origin: remote:<device-name>`.
- Join with existing local bookmarks merges them into the shared document by URL instead of discarding or forking.
- Storage additions: `identity.key`, `control.sqlite`. The bookmark document format is unchanged.
- **BREAKING**: a data directory created by `core-and-daemon` gains an identity and a device name on first start of the new daemon; nothing else changes.

## Capabilities

### New Capabilities
- `device-identity`: Ed25519 key per device, DeviceId derivation, key file storage, `SecureKeyStore` trait, device name.
- `trust-store`: pinned peer public keys, revocation, endpoint memory, discovery group secret and rotation in `control.sqlite`.
- `qr-pairing`: QR payload, pairing window, HMAC proof, fingerprint confirmation on both sides, provisioning, join-with-existing-data merge, rate limits and lockouts.
- `quic-sync-transport`: pinned-key QUIC/TLS 1.3 transport for the Automerge sync protocol, admission rules, connection limits, endpoint racing, reconnect.
- `peer-discovery`: private mDNS service advertising and browsing for address refresh only, manual endpoints, tailnet addresses.
- `sync-status`: per-peer reachability and last sync, sync-now, recovery semantics for missing or corrupt root document.

### Modified Capabilities
- `daemon-api`: new pairing, device and sync commands; `status` payload extended. (Delivered as ADDED requirements because the main spec is created by `core-and-daemon` in the same cycle.)
- `hooks`: `post-sync` event and `remote:<device>` origins. (ADDED requirements.)
- `cli`: pairing, devices, sync and device-name subcommands. (ADDED requirements.)

## Impact

- New dependencies (pinned like fi): `quinn =0.11.11` (runtime-tokio, rustls-ring), `rustls =0.23.45` (ring), `rcgen =0.14.5`, `x509-parser =0.18.0`, `ed25519-dalek =2.2.0`, `hkdf`, `hmac`, `sha2`, `mdns-sd =0.21.2`, `qrcode`, `base64`, `rand`, `zeroize`, `rusqlite` (already present).
- Source copied and renamed from `/home/gean/Sandboxes/sandbox/fi/crates/app_core/src/{identity,quinn_transport,discovery,discovery_control,routing,endpoint_memory}.rs` and the control tables in `adapters.rs`; pairing logic copied from `/home/gean/Sandboxes/sandbox/session-tap/crates/sessiontap-hub/src/{service,tls,endpoints}.rs` and adapted to QUIC streams.
- `wobookd` gains a UDP listener; firewall needs UDP 47390-47399 open on LAN and tailnet interfaces.
- Prepares `android-app`: the phone will link `wobook-sync` through UniFFI and implement `SecureKeyStore` with Android Keystore; the QR payload and handshake defined here are what the phone scans or shows.
