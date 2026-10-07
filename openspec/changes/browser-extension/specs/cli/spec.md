## ADDED Requirements

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
