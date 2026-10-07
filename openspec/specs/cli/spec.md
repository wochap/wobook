# cli Specification

## Purpose
Wobook command surface, output formats, editor flow, exit codes, behaviour when the daemon is down.

## Requirements

### Requirement: Command surface
The `wobook` CLI SHALL provide `add`, `edit`, `mv`, `rm`, `show`, `list`, `search`, `tags`, `import`, `export`, `status`, `hooks` and `completions` subcommands as thin clients of the daemon socket.

#### Scenario: Add with tags
- **WHEN** `wobook add example.com/x -t "ui library,react"` runs against a running daemon
- **THEN** the exit code is 0 and `wobook show https://example.com/x` prints tags `react` and `ui library`

#### Scenario: Help
- **WHEN** `wobook --help` runs
- **THEN** every subcommand above is listed

### Requirement: Daemon unavailable
When the socket cannot be connected, the CLI SHALL print a one-line message explaining how to start `wobookd` and exit with code 69.

#### Scenario: No daemon
- **WHEN** any data command runs with no daemon listening
- **THEN** stderr contains `wobookd is not running` and the exit code is 69

### Requirement: Output formats
`list` and `search` SHALL support `--format tsv|json|jsonl|pretty`. TSV SHALL print `url<TAB>title<TAB>tags` with tags comma-joined and any tab or newline inside a field replaced by a space. `pretty` SHALL print a buku-like block per bookmark. Default is `pretty` on a TTY and `tsv` otherwise.

#### Scenario: TSV for fzf
- **WHEN** `wobook list --format tsv` runs
- **THEN** each line has exactly two tabs and column 1 is the normalized URL

#### Scenario: JSON output
- **WHEN** `wobook show <url> --json` runs
- **THEN** stdout is one JSON object in the JSONL record shape

### Requirement: Editor flow
`wobook edit <url>` SHALL open `$EDITOR` (fallback `$VISUAL`, then `vi`) on a buku-style template with commented instructions and lines for URL, TITLE, TAGS (comma-separated) and a multi-line DESCRIPTION. On save, a changed URL line SHALL perform a rename, other changes an update, and an unchanged file SHALL do nothing. `wobook edit --new` SHALL open the template empty and perform an add on save; a blank TITLE SHALL trigger metadata fetch and `-` SHALL mean no title.

#### Scenario: Edit tags in editor
- **WHEN** `EDITOR` is a script that rewrites the TAGS line to `a, b`
- **THEN** after `wobook edit <url>` the bookmark has exactly tags `{a, b}`

#### Scenario: URL changed in editor
- **WHEN** the editor changes the URL line to a new URL
- **THEN** the CLI performs a rename and prints `moved <old> -> <new>`

#### Scenario: Editor aborted
- **WHEN** the editor exits non-zero
- **THEN** nothing is changed and the CLI exits 1

### Requirement: Exit codes
The CLI SHALL exit 0 on success, 1 on not found or invalid input, 2 on usage errors, 69 when the daemon is unavailable and 70 on internal errors.

#### Scenario: Not found
- **WHEN** `wobook show https://nope.example/` runs for an unknown URL
- **THEN** the exit code is 1 and stderr says not found

### Requirement: Delete and restore
`wobook rm <url>...` SHALL tombstone each URL and `wobook rm --restore <url>` SHALL undelete it.

#### Scenario: Restore
- **WHEN** a bookmark is removed and then `wobook rm --restore <url>` runs
- **THEN** `wobook list` shows it again with its previous tags

### Requirement: Pair subcommand
`wobook pair` SHALL call `pair.start`, print the QR code and the JSON payload, poll `pair.pending`, show the peer name and fingerprint, prompt `Trust this device? [y/N]`, and send `pair.confirm` or `pair.reject`. `wobook pair --join <json>` (or `-` to read stdin) SHALL call `pair.join` and run the same confirmation. `--yes` SHALL skip the prompt and `--json` SHALL print only the payload.

#### Scenario: Scripted pairing
- **WHEN** `wobook pair --json --yes` on A is piped into `wobook pair --join - --yes` on B
- **THEN** both exit 0 and `wobook devices list` on each shows the other

#### Scenario: Rejected at prompt
- **WHEN** the user answers `n`
- **THEN** the CLI sends `pair.reject`, prints that nothing was shared and exits 1

### Requirement: Devices subcommands
`wobook devices list [--json]` SHALL print name, platform, reachability, last synced and endpoints; `rename`, `revoke` and `add-endpoint` SHALL resolve devices by id or unique case-insensitive name. `revoke` SHALL ask `Revoke <name>? This cannot be undone. [y/N]` unless `--yes`.

#### Scenario: Revoke by name
- **WHEN** `wobook devices revoke pixel-8 --yes` runs
- **THEN** the device is revoked and `devices list` shows it as revoked

#### Scenario: Ambiguous name
- **WHEN** two trusted devices share a name and the name is used
- **THEN** the CLI exits 1 asking for an id prefix

### Requirement: Sync and device-name subcommands
`wobook sync status [--json]` SHALL print the local device line and one line per peer; `wobook sync now` SHALL call `sync.now`; `wobook device name [<name>]` SHALL get or set the name.

#### Scenario: Status table
- **WHEN** `wobook sync status` runs with one connected peer
- **THEN** the output contains the peer name, `lan` or `tailnet`, and a relative last-sync time

### Requirement: native-host subcommand
The `wobook` CLI SHALL provide `wobook native-host`, which runs the native messaging host loop on stdio until EOF, and `wobook native-host --print-manifest <firefox|chrome|brave> [--extension-id <id>] [--binary <path>]`, which prints a host manifest and exits without touching stdin.

#### Scenario: Listed in help
- **WHEN** `wobook --help` runs
- **THEN** `native-host` appears among the subcommands

#### Scenario: Manifest mode does not read stdin
- **WHEN** `wobook native-host --print-manifest firefox` runs with stdin attached to a terminal
- **THEN** it prints the manifest and exits 0 immediately

### Requirement: native-host exit codes
The host SHALL exit 0 on stdin EOF, 2 on usage errors, and 70 on unrecoverable I/O errors writing to stdout. A missing daemon SHALL NOT cause a non-zero exit.

#### Scenario: Stdout closed
- **WHEN** the browser closes the host's stdout while a response is pending
- **THEN** the host exits 70
