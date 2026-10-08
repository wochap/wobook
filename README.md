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

## Install with Nix

The flake exposes `packages.x86_64-linux.{wobook,wobookd,wobook-fzf,extension-firefox,extension-chromium}`
(`default` is `wobook`), `overlays.default` (adds `wobook`, `wobookd`, `wobook-fzf`) and
`homeManagerModules.wobook`.

```sh
nix run github:wochap/wobook -- --version
nix build github:wochap/wobook#wobookd
```

```nix
# flake.nix
inputs.wobook.url = "github:wochap/wobook";
inputs.wobook.inputs.nixpkgs.follows = "nixpkgs";
inputs.wobook.inputs.home-manager.follows = "home-manager"; # only used by checks

# home-manager config
imports = [ inputs.wobook.homeManagerModules.wobook ];
programs.wobook = {
  enable = true;
  fzf.enable = true;
  deviceName = "gdesktop";
  hooks."pre-add.strip-utm" = "${inputs.wobook}/contrib/hooks/pre-add.strip-utm";
  browsers = {
    firefox.enable = true;
    brave.enable = true;
    chromiumExtensionIds = [ "<id from chrome://extensions>" ];
  };
};
```

| Option | Default | Effect |
| --- | --- | --- |
| `enable` | `false` | install `wobook` (and `wobook-native-host`) |
| `package` / `daemonPackage` | flake packages | override the CLI / daemon |
| `deviceName` | `null` | `WOBOOK_DEVICE_NAME` for the service and session |
| `dataDir` | `null` | `WOBOOK_DATA_DIR` for the service and session |
| `daemon.enable` | `true` | `wobookd` systemd user service, `WantedBy=default.target` |
| `daemon.extraArgs` | `[]` | extra `wobookd` arguments |
| `hooks` | `{}` | name -> text or path, installed executable in `~/.config/wobook/hooks/` |
| `fzf.enable` | `false` | install `wobook-fzf` (bundles fzf, wl-clipboard, xdg-utils) |
| `shellCompletions.enable` | `true` | zsh/fish/bash completions; `false` installs `bin/` only |
| `browsers.{firefox,googleChrome,brave}.enable` | `false` | only writes the native messaging host manifest `dev.wochap.wobook.json`; does not install the extension |
| `browsers.chromiumExtensionIds` | `[]` | Chromium `allowed_origins`; required when a Chromium browser is enabled |
| `browsers.extraChromiumDirs` | `[]` | extra dirs relative to `~/.config` (e.g. `chromium`), get `NativeMessagingHosts/` |
| `browsers.extraFirefoxDirs` | `[]` | extra dirs relative to `~` (e.g. `.librewolf`), get `native-messaging-hosts/`; independent of `firefox.enable` |

Overlay: `import nixpkgs { overlays = [ inputs.wobook.overlays.default ]; }` gives `pkgs.wobook`.

### Installing the extension

The module does not install the browser extension. It writes the native messaging host
manifest, installs `wobook` and `wobook-native-host`, and runs `wobookd` (which must be running
for the extension to work). The extension bundles are unsigned and loaded by hand.

Firefox (extension id is fixed: `wobook@wochap.dev`):

```sh
nix build github:wochap/wobook#extension-firefox -o ff-ext
```

Open `about:debugging` → This Firefox → Load Temporary Add-on and pick
`ff-ext/wobook-<version>.xpi`. Temporary add-ons are removed when Firefox restarts; a permanent
install needs Firefox Developer Edition or Nightly with `xpinstall.signatures.required = false`
in `about:config`.

Chrome / Brave:

```sh
nix build github:wochap/wobook#extension-chromium -o chromium-ext
cp -rL chromium-ext ~/.local/share/wobook-extension
chmod -R u+w ~/.local/share/wobook-extension
```

The unpacked extension id is derived from its load path, so load it from a stable copy rather
than the store. Open `chrome://extensions` (or `brave://extensions`) → Developer mode → Load
unpacked → pick the copy, then add the shown id to `browsers.chromiumExtensionIds`, rebuild
home-manager and restart the browser. The option is a list because each Chromium-family
browser generates its own id.

Forks: Firefox forks such as LibreWolf use `browsers.extraFirefoxDirs = [ ".librewolf" ];`
(relative to `~`); Chromium forks use `browsers.extraChromiumDirs = [ "chromium" ];` (relative
to `~/.config`).

### Shell completions

The package ships zsh, fish and bash completions under `share/`. With home-manager,
`shellCompletions.enable = false` installs only `bin/`. Without the module, zsh picks up
`share/zsh/site-functions` through `fpath`, or generate them at startup:

```zsh
eval "$(wobook completions zsh)"
# or, with zsh-defer, after compinit:
zsh-defer eval "$(wobook completions zsh)"
```

`nix/package.nix` takes `withShellCompletions ? true`; override it through `callPackage`
(`pkgs.callPackage ./nix/package.nix { ...; withShellCompletions = false; }`) to build without
completions.

`nix flake check` runs fmt, clippy (`-D warnings`), the test suite and a home-manager evaluation
of the module.

## Migrate from buku

1. Add the `wobook` flake input and the module (see `contrib/nix-config/wobook/default.nix`
   for a `_custom.programs.wobook` wrapper in the buku module shape).
2. On one host set `_custom.programs.wobook.enable = true;`, rebuild, check
   `systemctl --user status wobookd`.
3. `wobook import ~/Sync/.config/buku/bookmarks.db`; compare `wobook list | wc -l` with buku's count.
4. In the kitty launcher `tui-bookmarks.sh` swap `buku-fzf {}` for `wobook-fzf {}`.
5. Set `_custom.programs.buku.enable = false;` on that host. The buku DB is untouched, so
   re-enabling buku rolls back.
6. On every other host: enable wobook, `wobook pair`, then disable buku.

Hooks that call `notify-send` run under the `wobookd` user service, which inherits the user
D-Bus (`DBUS_SESSION_BUS_ADDRESS`) from systemd; if notifications do not show, run
`systemctl --user import-environment DBUS_SESSION_BUS_ADDRESS` from your session startup.
