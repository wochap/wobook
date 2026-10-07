## ADDED Requirements

### Requirement: Single writer owns the data directory
`wobookd` SHALL be the only process that opens the Automerge storage and the read model. It SHALL hold an exclusive lock file (`daemon.lock`) for its lifetime and SHALL refuse to start when the lock is held.

#### Scenario: Second daemon refused
- **WHEN** a `wobookd` is running on a data directory and a second `wobookd` starts on the same directory
- **THEN** the second exits non-zero with a message naming the lock file, and the first keeps serving

#### Scenario: Stale socket replaced
- **WHEN** a previous daemon crashed leaving `wobookd.sock` behind and no lock holder
- **THEN** the new daemon unlinks the stale socket and binds successfully

### Requirement: Private unix socket with JSON-lines protocol
The daemon SHALL listen on a unix socket (default `$XDG_RUNTIME_DIR/wobook/wobookd.sock`, overridable) with mode 0600 inside a 0700 directory. Each connection SHALL carry one newline-terminated JSON request of at most 4 MiB and receive one newline-terminated JSON response, after which the daemon closes the connection.

#### Scenario: Ping
- **WHEN** a client sends `{"type":"ping"}`
- **THEN** the daemon replies `{"ok":true,"result":{"version":"<semver>"}}` and closes

#### Scenario: Malformed request
- **WHEN** a client sends invalid JSON or an unknown `type`
- **THEN** the daemon replies `{"ok":false,"error":{"code":"invalid_request",...}}` and stays running

#### Scenario: Oversized request
- **WHEN** a client sends more than 4 MiB without a newline
- **THEN** the daemon closes the connection without processing it

### Requirement: Command set
The daemon SHALL implement `ping`, `add`, `update`, `rename`, `delete`, `restore`, `get`, `list`, `search`, `tags`, `import`, `export`, `status` and `shutdown`, with bookmark payloads in the JSONL record shape.

#### Scenario: Add then get
- **WHEN** `add` is sent for a new URL with tags and then `get` for that URL
- **THEN** `get` returns the record with normalized URL, the tags and `deleted: false`

#### Scenario: Add existing without merge
- **WHEN** `add` is sent for a URL that already exists and is not deleted, without `merge: true`
- **THEN** the response is `{"ok":false,"error":{"code":"exists",...}}` and nothing changes

#### Scenario: Add existing with merge
- **WHEN** `add` is sent with `merge: true` for an existing URL with new tags
- **THEN** the tags are unioned, empty title or description are filled from the request, and the response reports `merged: true`

#### Scenario: Update tags incrementally
- **WHEN** `update` is sent with `add_tags: ["x"]` and `remove_tags: ["y"]`
- **THEN** the bookmark gains `x`, loses `y` and keeps its other tags

#### Scenario: Not found
- **WHEN** `get`, `update`, `delete` or `rename` targets a URL with no entry
- **THEN** the response error code is `not_found`

#### Scenario: Status
- **WHEN** `status` is sent
- **THEN** the result includes `version`, `data_dir`, `socket`, `bookmark_count`, `deleted_count`, `heads`, `uptime_s` and `hooks_dir`

#### Scenario: Shutdown
- **WHEN** `shutdown` is sent
- **THEN** the daemon flushes the Automerge document, replies ok, releases the lock and exits 0

### Requirement: Writes are durable before the response
The daemon SHALL flush the Automerge repository after every mutating command and reply only after the flush succeeds.

#### Scenario: Kill after response
- **WHEN** `add` returns ok and the daemon is killed with SIGKILL immediately afterwards
- **THEN** the next daemon start lists the added bookmark

### Requirement: Normalization applies at the boundary
Every URL and tag received by the daemon SHALL pass through normalization before lookup or storage, and responses SHALL carry the normalized values.

#### Scenario: Lookup with unnormalized URL
- **WHEN** a bookmark was added as `https://example.com/x` and `get` is sent with `EXAMPLE.com/x#frag`
- **THEN** the bookmark is found
