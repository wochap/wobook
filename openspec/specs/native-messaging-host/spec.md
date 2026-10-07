# native-messaging-host Specification

## Purpose
Native messaging host bridging the browser extension to the wobook daemon: framing, manifests, request routing.

## Requirements

### Requirement: Native messaging framing
`wobook native-host` SHALL read messages from stdin as a 4-byte little-endian length prefix followed by that many bytes of UTF-8 JSON, and SHALL write responses to stdout in the same framing. On stdin EOF it SHALL exit 0.

#### Scenario: Ping roundtrip
- **WHEN** a framed `{"type":"ping"}` is written to the host's stdin while `wobookd` is running
- **THEN** exactly one framed response is written containing `"ok":true` and the daemon version

#### Scenario: EOF ends the host
- **WHEN** stdin is closed after the last response
- **THEN** the host exits with status 0

### Requirement: Each frame is one daemon request
The host SHALL forward each parsed request to the daemon socket as a single JSON line, read one response line and return it unchanged in a frame. For `add` and `update` requests lacking `origin`, the host SHALL set `origin` to `extension`.

#### Scenario: Add through the host
- **WHEN** a framed `add` request without `origin` is sent
- **THEN** the daemon receives the request with `origin: "extension"` and the framed response is the daemon's response

#### Scenario: Responses preserve error codes
- **WHEN** the daemon answers with `hook_rejected`
- **THEN** the framed response carries the same `error.code` and `error.message`

### Requirement: Oversized and malformed frames are rejected without exiting
Frames longer than 4 MiB or whose body is not a valid request SHALL produce a framed error response (`invalid_request`) and the host SHALL continue reading the next frame. Daemon responses larger than 1 MiB SHALL be replaced by a framed `response_too_large` error.

#### Scenario: Oversized frame
- **WHEN** a frame declares a length of 5 MiB
- **THEN** the host writes an `invalid_request` error frame, skips the payload and answers a following `ping` normally

#### Scenario: Malformed JSON
- **WHEN** a frame body is `{not json`
- **THEN** the host writes an `invalid_request` error frame and keeps running

### Requirement: Daemon unavailable is reported as a response
When the daemon socket cannot be connected, the host SHALL write a framed `{"ok":false,"error":{"code":"daemon_unavailable","message":"wobookd is not running ..."}}` and keep running.

#### Scenario: Daemon stopped
- **WHEN** `wobookd` is not listening and a framed `get` is sent
- **THEN** the response error code is `daemon_unavailable` and the host does not exit

### Requirement: Host manifest printing
`wobook native-host --print-manifest <firefox|chrome|brave> [--extension-id <id>] [--binary <path>]` SHALL print a valid native messaging host manifest named `dev.wochap.wobook` with `type: "stdio"`, an absolute `path`, and `allowed_extensions: ["wobook@wochap.dev"]` for firefox or `allowed_origins: ["chrome-extension://<id>/"]` for chrome and brave. For chrome and brave `--extension-id` SHALL be required.

#### Scenario: Firefox manifest
- **WHEN** `wobook native-host --print-manifest firefox --binary /usr/bin/wobook-native-host` runs
- **THEN** stdout is JSON with `name` `dev.wochap.wobook`, `path` `/usr/bin/wobook-native-host` and `allowed_extensions` `["wobook@wochap.dev"]`

#### Scenario: Chrome manifest without id
- **WHEN** `wobook native-host --print-manifest chrome` runs without `--extension-id`
- **THEN** the command exits 2 with a usage error naming the missing flag

#### Scenario: Brave manifest
- **WHEN** `--print-manifest brave --extension-id abcdefghijklmnopabcdefghijklmnop` runs
- **THEN** `allowed_origins` is `["chrome-extension://abcdefghijklmnopabcdefghijklmnop/"]`

### Requirement: Wrapper script for manifests
`contrib/wobook-native-host` SHALL be an executable shell wrapper running `exec wobook native-host "$@"`, because host manifests accept a path without arguments.

#### Scenario: Wrapper forwards
- **WHEN** `contrib/wobook-native-host` is executed with `wobook` on PATH
- **THEN** it behaves identically to `wobook native-host`
