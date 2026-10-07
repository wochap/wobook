# peer-discovery Specification

## Purpose
Private mDNS service advertising and browsing for address refresh only, manual endpoints, tailnet addresses.

## Requirements

### Requirement: Private mDNS service
The daemon SHALL advertise `_wobook-<tag>._udp.local.` where `tag` is 10 lowercase base32 characters derived from the discovery group secret, with TXT records `v=1`, `id=<device_id>`, `port` and `epoch`, and SHALL browse the same type. Discovery SHALL be disableable with `WOBOOK_DISCOVERY=off`.

#### Scenario: Two devices on one LAN
- **WHEN** A and B share a group secret and run on the same LAN
- **THEN** each learns the other's current address and port within 30 seconds (manual check)

#### Scenario: Discovery disabled
- **WHEN** `WOBOOK_DISCOVERY=off` is set
- **THEN** no multicast socket is opened and remembered endpoints are still used

### Requirement: Discovery never grants trust
Addresses learned from mDNS SHALL only update `peer_endpoints` for device ids already present as trusted; records for unknown ids SHALL be ignored.

#### Scenario: Unknown advertiser
- **WHEN** a device with the group secret but no trusted row advertises
- **THEN** nothing is stored and no connection is attempted

### Requirement: Tailnet addresses
Addresses in `100.64.0.0/10` found on local interfaces SHALL be included in pairing payload endpoints and in the `hello` message so peers store them as `kind = tailnet`.

#### Scenario: Tailnet endpoint shared
- **WHEN** A has `tailscale0` with `100.84.12.7` and pairs with B
- **THEN** B stores `100.84.12.7:<port>` for A with `kind = tailnet`

### Requirement: Secret rotation propagates
When the group secret rotates, connected trusted peers SHALL receive the new secret and epoch over the control stream, acknowledge, switch their advertised service type, and keep the previous secret for 24 hours. Peers connecting later SHALL receive the current secret in `hello`.

#### Scenario: Offline peer catches up
- **WHEN** A rotates while C is offline and C connects two hours later
- **THEN** C adopts the new epoch from A's `hello`
