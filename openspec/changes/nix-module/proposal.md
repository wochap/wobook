## Why

The earlier changes deliver binaries, a daemon, hooks, an fzf wrapper and a browser extension, but nothing installs them. The user's machines are NixOS hosts configured from `/home/gean/nix-config`, where buku is enabled per host through a home-manager wrapper module. wobook replaces buku only when it can be enabled the same way: one flake input, one `enable = true`, and the daemon, hooks, fzf script, shell completions and browser native-messaging manifests appear on the host. This change packages everything with Nix and ships a ready-to-copy nix-config module, without touching the existing buku module.

## What Changes

- `flake.nix` gains real outputs: `packages.x86_64-linux.{wobook, wobookd, wobook-fzf, extension-firefox, extension-chromium}`, `packages.default = wobook`, `overlays.default`, `homeManagerModules.wobook`, and `checks` (crane cargo build/test/clippy/fmt, home-manager module evaluation). Existing dev shells (`default` rust, `android`, `android-emulator`) stay.
- Rust crates are built with crane for incremental dependency caching and per-crate check derivations.
- New home-manager module `homeManagerModules.wobook` with options `enable`, `package`, `deviceName`, `dataDir`, `daemon.enable`, `hooks`, `fzf.enable`, `browsers.{firefox,googleChrome,brave}.enable`, `browsers.chromiumExtensionIds`, `shellCompletions.enable`. It installs the systemd user service `wobookd.service`, hook scripts, the fzf wrapper with its runtime tools, native messaging host manifests and shell completions.
- `contrib/nix-config/wobook/default.nix`: example module in the user's `_custom.programs.<name>` / `_custom.hm` shape for copying into `/home/gean/nix-config/modules/shared/programs/cli/wobook/`, with comments covering the flake input line and disabling buku per host.
- README migration section: add input, enable module on one host, import the buku database, swap the kitty keybinding from `buku-fzf` to `wobook-fzf`, disable buku on that host, repeat after pairing.
- Android APK is not built by Nix; the Gradle build stays in the `android` dev shell.

## Capabilities

### New Capabilities
- `nix-packages`: flake package outputs, crane build, overlay, extension bundles, version and metadata.
- `home-manager-module`: the `homeManagerModules.wobook` options and what each one installs (daemon unit, hooks, fzf, completions, device name).
- `browser-host-manifests`: native messaging host manifest generation and installation paths for Firefox, Google Chrome and Brave.
- `nix-ci-checks`: `nix flake check` contents, including the home-manager module evaluation check.

### Modified Capabilities
<!-- none: packaging only, runtime requirements unchanged -->

## Impact

- Files: `flake.nix`, `flake.lock`, new `nix/` directory (package, module, checks), `contrib/nix-config/wobook/default.nix`, `README.md`.
- New flake inputs: `crane`, `flake-utils` or plain `nixpkgs.lib` system loop (decide in design), `home-manager` (checks only, `inputs.home-manager.follows` left to the consumer).
- Depends on artifacts from `core-and-daemon` (`wobook`, `wobookd`, `contrib/wobook-fzf.sh`, `contrib/hooks`), `browser-extension` (`extension/` sources, extension ids, `wobook native-host` subcommand) and `p2p-sync` (`deviceName` env/flag).
- User's `/home/gean/nix-config` is not edited by this change; it only receives a copyable module and documentation.
