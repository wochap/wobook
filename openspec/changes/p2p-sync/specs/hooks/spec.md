## ADDED Requirements

### Requirement: post-sync event
After a peer session changes the document heads, the daemon SHALL run `post-sync` hooks with a payload `{"event":"post-sync","peer":{"id","name"},"changed":<count>,"heads":"<heads string>"}` and `WOBOOK_EVENT=post-sync`, `WOBOOK_PEER=<name>`.

#### Scenario: Hook fires once per session
- **WHEN** B pushes three bookmark changes to A in one sync session
- **THEN** A runs `post-sync` once with `changed = 3`, after the three per-bookmark hooks

#### Scenario: No change, no hook
- **WHEN** a sync session exchanges messages but heads do not change
- **THEN** `post-sync` does not run

### Requirement: Remote origins
`post-add`, `post-update` and `post-delete` hooks triggered by remote changes SHALL carry `origin = "remote:<peer name>"` in the payload and `WOBOOK_ORIGIN`.

#### Scenario: Notify only on remote adds
- **WHEN** the shipped `post-add.notify` hook is installed and B adds a bookmark that syncs to A
- **THEN** `notify-send` runs on A, and it does not run when A adds a bookmark locally
