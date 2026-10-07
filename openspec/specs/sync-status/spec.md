# sync-status Specification

## Purpose
Per-peer reachability and last sync, sync-now, recovery semantics for missing or corrupt root document.

## Requirements

### Requirement: Per-peer sync status
`sync.status` SHALL report for each trusted device its reachability (`lan`, `tailnet` or `unreachable` based on the kind of the connected or last successful endpoint), `connected`, `in_progress`, `last_sync_ms`, `heads_equal` and `last_error`, plus the local device id, name, port and discovery epoch.

#### Scenario: Connected and converged
- **WHEN** A and B are connected with equal heads
- **THEN** A's `sync.status` lists B with `connected = true`, `heads_equal = true` and a recent `last_sync_ms`

#### Scenario: Unreachable
- **WHEN** B is offline
- **THEN** A lists B with `reachability = unreachable`, `connected = false` and the previous `last_sync_ms`

### Requirement: Recovery of the root document
On start, a missing document with a joined bootstrap record SHALL enter a joining state and fetch from any reachable peer; an unreadable document SHALL be moved to `quarantine/<timestamp>/`, counted in `recovery_attempts`, and refetched; after 3 attempts the daemon SHALL stop retrying and report `recovery = needs_attention` in `status`. Nothing SHALL be deleted automatically.

#### Scenario: Document deleted on a joined device
- **WHEN** B's document file is removed and B restarts while A is reachable
- **THEN** B lists all bookmarks again after sync and `status.recovery` is absent

#### Scenario: Corrupt document
- **WHEN** B's document file is truncated and B restarts
- **THEN** the file is found under `quarantine/`, `recovery_attempts` is 1, and B resyncs from A

#### Scenario: Attempt limit
- **WHEN** recovery fails three times
- **THEN** `status.recovery` equals `needs_attention` and no further automatic attempts run

### Requirement: Remote changes reach the read model and hooks
A `DocumentEvent` with remote origin SHALL trigger read-model reconciliation, then `post-add`, `post-update` or `post-delete` hooks per changed bookmark with `origin = remote:<peer name>`, then one `post-sync` hook.

#### Scenario: Remote add visible locally
- **WHEN** B adds a bookmark while connected to A
- **THEN** A's `list` shows it within 5 seconds and A's `post-add` hook ran with `WOBOOK_ORIGIN = remote:B`
