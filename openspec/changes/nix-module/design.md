## Context

The flake from `core-and-daemon` only has dev shells. The user's NixOS config (`/home/gean/nix-config`) wires programs through a custom option namespace: `/home/gean/nix-config/modules/shared/programs/cli/buku/default.nix` declares `options._custom.programs.buku.enable = lib.mkEnableOption {}`, and under `config = lib.mkIf cfg.enable` sets `_custom.hm = { home.packages = [...]; home.symlinks = {...}; }` where `_custom.hm` is their home-manager passthrough. buku's flake input is `buku.url = "github:jarun/buku/v5.1.1"; buku.flake = false;` and `session-tap.url = "github:wochap/session-tap"` shows how they consume their own flakes. Hosts enable programs with `_custom.programs.buku.enable = true;` (`hosts/gdesktop/default.nix:53`). The kitty launcher `tui-bookmarks.sh` runs `buku-fzf {}`.

session-tap's flake builds with `rustPlatform.buildRustPackage` and `doCheck = false`; its `android` / `android-emulator` shells were copied into this repo by `android-app`. Rules: the buku module is never modified; the user disables it per host and adds a new `wobook` module folder.

## Goals / Non-Goals

**Goals:**
- `nix build .#wobook` produces a store path with `bin/wobook`, `bin/wobookd` is in `.#wobookd`, `.#wobook-fzf` wraps the script with its runtime deps on PATH.
- One home-manager module turns on daemon, hooks, fzf, completions and browser host manifests.
- A copy-paste module for the user's config that mirrors the buku module shape.
- `nix flake check` catches Rust regressions and module option typos.

**Non-Goals:**
- Building the Android APK with Nix (Gradle offline builds are out of scope; `nix develop .#android` remains the way).
- NixOS system module, socket activation, multi-user installs, non-x86_64-linux platforms (aarch64-linux may be added later by a system list, but is not tested).
- Editing `/home/gean/nix-config`.
- Signing the Firefox extension with AMO; the package produces an unsigned `.xpi`/zip for self-distribution.

## Decisions

### D1. crane over rustPlatform.buildRustPackage

crane (`inputs.crane.url = "github:ipetkov/crane"`) splits the build into `cargoArtifacts` (dependencies only, cached across source changes) and per-crate `buildPackage` derivations, and gives `cargoClippy`, `cargoFmt`, `cargoNextest`/`cargoTest` derivations that share the same artifacts for `checks`. `buildRustPackage` rebuilds every dependency on any source change and has no clippy/fmt helpers. Cost: one more input and `craneLib.cleanCargoSource` filtering. Pinned with `crane.inputs.nixpkgs.follows = "nixpkgs"`.

Common args in `nix/rust.nix`:
```nix
{ src = craneLib.cleanCargoSource ./..; strictDeps = true;
  nativeBuildInputs = [ pkgs.pkg-config ]; buildInputs = [ pkgs.sqlite ];
  # rustls everywhere, no openssl
  WOBOOK_GIT_REV = self.shortRev or "dirty"; }
```
`packages.wobook` = `buildPackage` with `cargoExtraArgs = "-p wobook"`, `packages.wobookd` = same with `-p wobookd`. Both reuse `cargoArtifacts`. `meta.mainProgram` set so `nix run` works.

### D2. System handling

No `flake-utils`; a small `forAllSystems` over `[ "x86_64-linux" ]` with `nixpkgs.lib.genAttrs`, matching session-tap's single-system style. The HM module and overlay are system-agnostic attributes.

### D3. Package set

| Output | Content |
| --- | --- |
| `wobook` | `bin/wobook`, zsh/fish/bash completions generated at build time with `wobook completions <shell>` into `share/` |
| `wobookd` | `bin/wobookd` |
| `wobook-fzf` | `pkgs.writeShellApplication { name = "wobook-fzf"; runtimeInputs = [ wobook fzf wl-clipboard xdg-utils coreutils ]; text = readFile ../contrib/wobook-fzf.sh; }` (writeShellApplication runs shellcheck at build) |
| `extension-firefox` | `web-ext build` output `wobook-<version>.zip` renamed `.xpi`, from `extension/` with the Firefox manifest |
| `extension-chromium` | unpacked extension directory for "Load unpacked" or enterprise policy, from `extension/` with the Chromium manifest |
| `default` | `wobook` |

Extension packages use `pkgs.buildNpmPackage` only if the extension has a build step; if `extension/` is plain JS (decided in `browser-extension`), they are `runCommand` copies plus `web-ext` from nixpkgs for the Firefox zip. The design leaves both branches; tasks say "match what `extension/` actually is".

`overlays.default = final: prev: { wobook = ...; wobookd = ...; wobook-fzf = ...; }` built via `final.callPackage nix/package.nix {}` so consumers can override.

### D4. Home-manager module (`nix/hm-module.nix`)

Imported as `homeManagerModules.wobook = import ./nix/hm-module.nix { inherit self; }` so it can default `package` to the flake's package for the host's system.

```nix
options.programs.wobook = {
  enable = mkEnableOption "wobook";
  package = mkOption { type = package; default = self.packages.${pkgs.system}.wobook; };
  daemonPackage = mkOption { type = package; default = self.packages.${pkgs.system}.wobookd; };
  deviceName = mkOption { type = nullOr str; default = null; };     # -> WOBOOK_DEVICE_NAME env on the service and in sessionVariables
  dataDir = mkOption { type = nullOr str; default = null; };        # -> WOBOOK_DATA_DIR
  daemon.enable = mkOption { type = bool; default = true; };
  daemon.extraArgs = mkOption { type = listOf str; default = []; };
  hooks = mkOption { type = attrsOf (either str path); default = {}; };  # name -> xdg.configFile."wobook/hooks/<name>" executable = true
  fzf.enable = mkEnableOption "wobook-fzf wrapper";
  shellCompletions.enable = mkOption { type = bool; default = true; };
  browsers.firefox.enable = mkEnableOption ...; browsers.googleChrome.enable = ...; browsers.brave.enable = ...;
  browsers.firefoxExtensionId = mkOption { type = str; default = "wobook@wochap.dev"; };
  browsers.chromiumExtensionIds = mkOption { type = listOf str; default = [ "<id from browser-extension change>" ]; };
};
```
Effects:
- `home.packages = [ package ] ++ optional fzf.enable fzfPackage` (fzf package already bundles fzf, wl-clipboard, xdg-utils).
- `systemd.user.services.wobookd` when `daemon.enable`: `Unit.Description`, `Service.ExecStart = "${daemonPackage}/bin/wobookd ${escapeShellArgs extraArgs}"`, `Restart = "on-failure"`, `RestartSec = 2`, `Environment` from `deviceName`/`dataDir` when set, `Install.WantedBy = [ "default.target" ]`. The service starts before the user's graphical session so the kitty keybinding never hits exit 69.
- `home.sessionVariables.WOBOOK_DATA_DIR` / `WOBOOK_DEVICE_NAME` so the CLI and daemon agree.
- `xdg.configFile."wobook/hooks/${name}" = { source or text; executable = true; }` per hook.
- Completions: `wobook` package ships them under `share/{zsh/site-functions,fish/vendor_completions.d,bash-completion/completions}`; home-manager picks them up when the shells are enabled, so `shellCompletions.enable = false` filters them out with `lib.hiPrio`/a wrapper that drops `share/`. Simpler: when false, install `wobook` through a `symlinkJoin` that only exposes `bin/`.
- Browser manifests: see D5.
- `assertions`: `fzf.enable -> enable`, at least one chromium id when a chromium browser is enabled.

The module name is `programs.wobook`, the standard HM namespace; the user's `_custom.programs.wobook` wrapper (D6) forwards to it.

### D5. Native messaging host manifests

Host name `dev.wochap.wobook` (matches the Android namespace `com.wochap`). Manifest content:
```json
{ "name": "dev.wochap.wobook", "description": "wobook native messaging host",
  "path": "<store path>/bin/wobook", "type": "stdio",
  "allowed_extensions": ["wobook@wochap.dev"] }          // Firefox
{ ..., "allowed_origins": ["chrome-extension://<id>/"] } // Chromium family
```
`path` points at `${package}/bin/wobook`; the extension's host manifest from `browser-extension` names the subcommand through the binary's argv-less detection (`wobook native-host` when launched by a browser is identified by the browser passing the extension origin as argv[1] on Chromium and the manifest path on Firefox; the CLI from `browser-extension` already handles that detection, so `path` is the plain binary). If `browser-extension` instead requires an explicit wrapper, the module uses `pkgs.writeShellScript "wobook-native-host" ''exec ${package}/bin/wobook native-host "$@"''`; the task says to check `extension/` docs and pick.

Install paths (home-manager `home.file`):
| Browser | Path |
| --- | --- |
| Firefox | `~/.mozilla/native-messaging-hosts/dev.wochap.wobook.json` |
| Google Chrome | `~/.config/google-chrome/NativeMessagingHosts/dev.wochap.wobook.json` |
| Brave | `~/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts/dev.wochap.wobook.json` |

Chromium itself (`~/.config/chromium/...`) is not enabled by an option because the user does not run it; a `browsers.extraChromiumDirs` list of relative config dirs covers it and any flatpak variant without new options.

### D6. Example nix-config module (`contrib/nix-config/wobook/default.nix`)

Mirrors `buku/default.nix` shape:
```nix
{ config, lib, inputs, ... }:
let cfg = config._custom.programs.wobook; inherit (config._custom.globals) userName; in {
  options._custom.programs.wobook.enable = lib.mkEnableOption { };
  config = lib.mkIf cfg.enable {
    _custom.hm = {
      imports = [ inputs.wobook.homeManagerModules.wobook ];
      programs.wobook = {
        enable = true; daemon.enable = true; fzf.enable = true;
        deviceName = config.networking.hostName;
        browsers = { firefox.enable = true; googleChrome.enable = true; brave.enable = true; };
        hooks."pre-add.strip-utm" = "${inputs.wobook}/contrib/hooks/pre-add.strip-utm";
      };
    };
  };
}
```
Header comments: add `wobook.url = "github:wochap/wobook"; wobook.inputs.nixpkgs.follows = "nixpkgs";` to `flake.nix`, add `./programs/cli/wobook` to `modules/shared/default.nix` imports, per host set `_custom.programs.buku.enable = false; _custom.programs.wobook.enable = true;`, swap `buku-fzf` for `wobook-fzf` in `tui-bookmarks.sh`. Whether `_custom.hm` accepts `imports` must be verified against their wrapper; the comment shows the alternative of importing the HM module from `home-manager.sharedModules`.

### D7. Checks

`checks.x86_64-linux`:
- `wobook-clippy` (`craneLib.cargoClippy` with `--all-targets -- -D warnings`), `wobook-fmt` (`cargoFmt`), `wobook-test` (`cargoTest` or `cargoNextest`; e2e tests spawn binaries from `target/` so set `CARGO_TARGET_DIR` default and allow network-free tests; metadata-fetch tests use the in-process fixture), `wobook-doc` optional.
- `hm-module-eval`: `inputs.home-manager.lib.homeManagerConfiguration { modules = [ self.homeManagerModules.wobook { home = { username = "test"; homeDirectory = "/home/test"; stateVersion = "25.05"; }; programs.wobook = { enable = true; fzf.enable = true; deviceName = "ci"; hooks.x = "#!/bin/sh\nexit 0"; browsers = { firefox.enable = true; googleChrome.enable = true; brave.enable = true; }; }; } ]; }` and expose `.activationPackage` as the check. This evaluates and builds the generated files, catching typos and bad paths.
- `shellcheck` is implicit in `writeShellApplication`.
`home-manager` becomes a flake input used only by checks; `README` tells consumers they can `inputs.wobook.inputs.home-manager.follows = "home-manager"`.

### D8. Version and reproducibility

Crate version from `Cargo.toml` via `craneLib.crateNameFromCargoToml`; `wobook --version` prints `<version> (<git short rev>)` with rev from `self.shortRev or self.dirtyShortRev or "unknown"` passed as `WOBOOK_GIT_REV` build env and read with `option_env!` (already the pattern in `core-and-daemon` status output, extend if missing).

## Risks / Trade-offs

- [crane input churn and cleanCargoSource dropping non-Rust files the build needs (e.g. `contrib/` read at build time)] → only `wobook-fzf` reads `contrib/`, and it uses `./contrib/wobook-fzf.sh` directly, not the crane-filtered source.
- [e2e tests inside the Nix sandbox: no network, no `$XDG_RUNTIME_DIR`] → tests already use temp dirs and `WOBOOK_SOCKET`; set `XDG_RUNTIME_DIR=$TMPDIR` in the check; fetch tests use the local fixture only.
- [`_custom.hm` may not accept `imports`] → documented alternative in the example module; the apply agent cannot verify against the user's config.
- [Chromium extension id unknown until the extension is packed] → option with default from `browser-extension`'s documented key; user can override.
- [Service starts before Wayland session, `notify-send` hooks need `DBUS_SESSION_BUS_ADDRESS`] → systemd user services inherit the user bus; documented.
- [Home-manager version drift in the check] → `home-manager` input follows nixpkgs, check pinned by `flake.lock`.

## Migration Plan

1. In `/home/gean/nix-config/flake.nix` add the `wobook` input; copy `contrib/nix-config/wobook/default.nix` to `modules/shared/programs/cli/wobook/default.nix`; add it to `modules/shared/default.nix`.
2. On one host: `_custom.programs.wobook.enable = true;`, rebuild, `systemctl --user status wobookd`.
3. `wobook import ~/Sync/.config/buku/bookmarks.db`; verify `wobook list | wc -l` equals buku's count.
4. Change `tui-bookmarks.sh` to call `wobook-fzf`; set `_custom.programs.buku.enable = false;` on that host.
5. Other hosts: enable wobook, pair (`wobook pair`), then disable buku. Rollback: re-enable buku; its DB is untouched.

## Open Questions

- Exact Chromium extension id and whether the native host needs a wrapper script: resolved by reading `extension/` and `browser-extension` specs during apply.
