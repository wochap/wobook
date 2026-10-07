## ADDED Requirements

### Requirement: Hook discovery
The daemon SHALL run executable files from `$XDG_CONFIG_HOME/wobook/hooks/` (overridable by `WOBOOK_HOOKS_DIR`) whose name equals an event name or starts with `<event>.`, in lexical order. Non-executable files SHALL be ignored.

#### Scenario: Multiple hooks for one event
- **WHEN** `post-add.10-log` and `post-add.20-notify` exist and are executable
- **THEN** both run, `10-log` first

#### Scenario: Missing directory
- **WHEN** the hooks directory does not exist
- **THEN** commands succeed and no hook is attempted

### Requirement: Hook payload
Each hook SHALL receive on stdin one JSON object with `event`, `origin`, `bookmark` (JSONL record) and `previous` (record or null), and the environment variables `WOBOOK_EVENT`, `WOBOOK_ORIGIN`, `WOBOOK_URL` and `WOBOOK_DATA_DIR`.

#### Scenario: Payload content
- **WHEN** a `post-update` hook runs after a title change
- **THEN** `bookmark.title` is the new title and `previous.title` is the old title

### Requirement: pre-add can rewrite or reject
`pre-add` hooks SHALL run before the metadata fetch and before storage. Exit 0 with non-empty stdout that parses as a record SHALL replace the incoming bookmark (including an optional `fetch: false`); exit 0 with empty stdout SHALL accept unchanged; non-zero exit SHALL reject the add with error code `hook_rejected` and the hook's stderr as message. A hook exceeding 10 seconds SHALL count as a rejection.

#### Scenario: Strip tracking parameters
- **WHEN** the shipped `pre-add.strip-utm` hook is installed and `wobook add 'https://e.example/?a=1&utm_source=x'` runs
- **THEN** the stored URL is `https://e.example/?a=1`

#### Scenario: Reject
- **WHEN** a `pre-add` hook exits 1 with `blocked` on stderr
- **THEN** the add fails with `hook_rejected` and message `blocked`, and nothing is stored

### Requirement: post hooks are isolated
`post-add`, `post-update`, `post-delete` and `post-sync` SHALL run after the write is durable, sequentially in a background task, with a 10 second timeout, and their failure or output SHALL never change the command result.

#### Scenario: Failing post hook
- **WHEN** a `post-add` hook exits 1
- **THEN** the `add` command still returns ok and the failure is logged

#### Scenario: Origin visible to hooks
- **WHEN** a bookmark is added with `origin: "cli"`
- **THEN** `WOBOOK_ORIGIN` is `cli` for its `post-add` hooks

### Requirement: Hook replay for testing
`wobook hooks run <event> <url>` SHALL invoke the hooks for the given event with the current record as payload and print each hook's exit code and output.

#### Scenario: Replay
- **WHEN** `wobook hooks run post-add <url>` runs
- **THEN** every matching hook executes once and its exit code is printed
