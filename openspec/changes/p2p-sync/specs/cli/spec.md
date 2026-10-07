## ADDED Requirements

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
