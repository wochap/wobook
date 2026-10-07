# wobook

Local-first bookmarks. An Automerge document is the source of truth; a SQLite
read model is rebuilt from it and can be deleted at any time. `wobookd` is the
only process that touches the data; `wobook` is a thin CLI over its socket.

## Install

```sh
nix develop -c cargo build --release
install -m755 target/release/wobookd target/release/wobook ~/.local/bin/
```

Run the daemon by hand (`wobookd`) or as a user service with
`contrib/wobookd.service` (edit `ExecStart` to where `wobookd` lives).

## Locations

| What | Default | Override |
| --- | --- | --- |
| Data (Automerge, read model, lock) | `$XDG_DATA_HOME/wobook` | `WOBOOK_DATA_DIR`, `wobookd --data-dir` |
| Socket | `$XDG_RUNTIME_DIR/wobook/wobookd.sock` | `WOBOOK_SOCKET`, `--socket` |
| Hooks | `$XDG_CONFIG_HOME/wobook/hooks` | `WOBOOK_HOOKS_DIR`, `wobookd --hooks-dir` |
| Device key (Ed25519 seed, mode 0600) | `<data>/identity.key` | |
| Trust store, endpoints, device name, sync port | `<data>/control.sqlite` | |
| Quarantined documents, pre-join exports | `<data>/quarantine/`, `<data>/pre-join-<ts>.jsonl` | |

Sync environment: `WOBOOK_SYNC_PORT` forces the UDP port, `WOBOOK_DISCOVERY=off`
disables mDNS, `WOBOOK_DEVICE_NAME` sets the device name at start,
`WOBOOK_SYNC_LOOPBACK=1` also offers `127.0.0.1` in pairing payloads.

## Usage

```sh
wobook import ~/.local/share/buku/bookmarks.db   # buku DB, .jsonl or Netscape .html
wobook add example.com/x -t "ui library,react"
wobook search shcn ui
wobook edit https://example.com/x                # $EDITOR; changing the URL moves it
wobook rm https://example.com/x                  # --restore to undo
wobook export backup.jsonl
```

The bookmark identity is the normalized URL (lowercase scheme and host, no
default port, no fragment except `#!`). Tags are comma-separated and may contain
spaces.

## Hooks

Executable files named `<event>` or `<event>.<suffix>` in the hooks directory
run in lexical order for `pre-add`, `post-add`, `post-update`, `post-delete` and
`post-sync`. They get `{"event","origin","bookmark","previous"}` on stdin and
`WOBOOK_EVENT`, `WOBOOK_ORIGIN`, `WOBOOK_URL`, `WOBOOK_DATA_DIR` in the
environment. Changes that arrive from a peer run `post-add`/`post-update`/
`post-delete` with origin `remote:<device name>` and `WOBOOK_PEER`, followed by
one `post-sync` per sync batch with `{"event","peer":{"id","name"},"changed","heads"}`
(skipped when nothing changed). A `pre-add` hook may print a replacement record (optionally with
`"fetch": false`) or exit non-zero to reject the add (stderr is the message).
`post-*` hooks run in the background with a 10 s timeout and never affect the
result. Examples live in `contrib/hooks/`; try one with
`wobook hooks run post-add <url>`.

## Sync between devices

Every device is a peer; there is no server. Devices pair once, then replicate
the Automerge document over QUIC (UDP 47390-47399, first free port, persisted)
on the LAN or over Tailscale. Peers are authenticated by pinned Ed25519 keys;
mDNS only refreshes addresses of devices that are already trusted.

Desktop to desktop:

```sh
# on the first machine: prints a QR code and a JSON line, valid 120 s
wobook pair
# on the second machine: paste that JSON line
wobook pair --join -
```

Both sides show the peer name and a fingerprint (`1a2b3c4d 5e6f7a8b ...`);
compare them and answer `Trust this device? [y/N]` on both. `--yes` skips the
prompt for scripted setups (weaker: no human checks the fingerprint). Desktop to
phone: run `wobook pair` and scan the QR code with the app, or let the phone
show its code and paste the JSON into `wobook pair --join -`.

A device that already has bookmarks can join: its bookmarks are exported to
`pre-join-<ts>.jsonl`, the old document is moved to `quarantine/pre-join-<ts>/`,
the shared document is fetched and the export is merged back by URL. A device
that is already paired into another mesh refuses to join a different one; move
its data directory away first.

```sh
wobook devices list                     # name, platform, reachability, last sync, endpoints
wobook devices rename <id|name> <name>
wobook devices revoke <id|name>         # permanent; rotates the discovery secret
wobook devices add-endpoint <id|name> 100.84.12.7:47390
wobook sync status                      # this device and every peer
wobook sync now                         # dial every peer now
wobook device name [<name>]
```

Firewall: allow inbound UDP 47390-47399 on the LAN interface and on
`tailscale0`; mDNS uses the standard multicast group on 5353. On NixOS:

```nix
networking.firewall.interfaces."tailscale0".allowedUDPPortRanges = [ { from = 47390; to = 47399; } ];
networking.firewall.interfaces."enp3s0".allowedUDPPortRanges = [ { from = 47390; to = 47399; } ];
```

Tailscale: tailnet addresses (`100.64.0.0/10`) are included in the pairing
payload and in the hello each peer sends on connect, so peers reach each other
off-LAN without discovery. Multicast does not cross the tailnet; when an address
changes, `wobook sync now` or `devices add-endpoint` helps. No relays or NAT
traversal are attempted.

Recovery on start (nothing is ever deleted automatically):

| State | What happens |
| --- | --- |
| Joined, document file missing | fetched again from any reachable peer; `status.recovery` set until done |
| Document file unreadable | moved to `quarantine/`, attempt counted, fetched again; after 3 attempts the daemon refuses to start (`needs_attention`) |
| `control.sqlite` unreadable | daemon exits with the path |
| `identity.key` missing but `control.sqlite` has an identity | daemon exits: peers pinned the old key; move the data directory away and pair again |

mDNS between two real hosts on the LAN and over Tailscale: not yet verified
manually.

## fzf

`contrib/wobook-fzf.sh` replaces `buku-fzf`: `--select` copies a URL with
`wl-copy`, `--open` runs `xdg-open`, `--add` opens an empty editor template,
`--edit` loops over edits.
