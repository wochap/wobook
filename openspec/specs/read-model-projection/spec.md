# read-model-projection Specification

## Purpose
Disposable SQLite read model rebuilt from Automerge heads, checkpointing, recovery by deletion.

## Requirements

### Requirement: SQLite read model is disposable
The system SHALL maintain a SQLite read model (`read-model.sqlite`) with tables `meta`, `bookmarks` and `tags` derived entirely from the Automerge document. Deleting the file SHALL never lose data.

#### Scenario: Read model deleted while daemon stopped
- **WHEN** `read-model.sqlite` is removed and the daemon starts
- **THEN** the daemon rebuilds it from the Automerge document and `list` returns every non-deleted bookmark

#### Scenario: Schema version mismatch
- **WHEN** the read model's `meta.schema_version` differs from the daemon's expected version
- **THEN** the daemon discards and rebuilds the read model without error

### Requirement: Projection is checkpointed by Automerge heads
The projection SHALL store the sorted document heads as `meta.heads_checkpoint` and SHALL skip the rebuild when the current heads match the checkpoint.

#### Scenario: No change, no rebuild
- **WHEN** `reconcile` runs twice without any document change in between
- **THEN** the second run performs no writes to the read model

#### Scenario: Local change triggers rebuild
- **WHEN** a bookmark is added through the daemon
- **THEN** a subsequent `list` reflects it and the checkpoint equals the new heads

### Requirement: Projection is atomic
A rebuild SHALL run inside a single SQLite transaction so readers never observe a partially populated read model.

#### Scenario: Rebuild of many records
- **WHEN** the document holds 5,000 bookmarks and a rebuild runs
- **THEN** the rebuild completes in one transaction and finishes in under 2 seconds on the development machine
