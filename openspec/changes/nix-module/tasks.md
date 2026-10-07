## 1. Flake inputs and structure

- [ ] 1.1 Add `crane` and `home-manager` inputs (both `inputs.nixpkgs.follows = "nixpkgs"`) to `flake.nix`; replace any flake-utils usage with a `forAllSystems` over `[ "x86_64-linux" ]`; keep the `default`, `android` and `android-emulator` dev shells working.
- [ ] 1.2 Create `nix/rust.nix` with crane common args (`cleanCargoSource`, `strictDeps`, `pkg-config`, `sqlite`, `WOBOOK_GIT_REV` from `self.shortRev or self.dirtyShortRev or "unknown"`) and the shared `cargoArtifacts`.
- [ ] 1.3 Make `wobook --version` print `<version> (<rev>)` by reading `option_env!("WOBOOK_GIT_REV")`, if `core-and-daemon` did not already.

## 2. Packages

- [ ] 2.1 Define `packages.wobook` (`-p wobook`, `meta.mainProgram`, postInstall generating zsh/fish/bash completions via `wobook completions`) and `packages.wobookd` (`-p wobookd`) in `nix/package.nix`, reusing `cargoArtifacts`; set `packages.default = wobook`.
- [ ] 2.2 Define `packages.wobook-fzf` with `writeShellApplication` over `contrib/wobook-fzf.sh` and runtime inputs `wobook fzf wl-clipboard xdg-utils coreutils`; fix any shellcheck findings in the script.
- [ ] 2.3 Define `packages.extension-firefox` (`web-ext build` to an unsigned `.xpi`) and `packages.extension-chromium` (unpacked directory) from `extension/`, matching whether `extension/` has a build step (npm) or is plain files.
- [ ] 2.4 Define `overlays.default` exposing `wobook`, `wobookd`, `wobook-fzf` via `callPackage`.
- [ ] 2.5 Verify: `nix build .#wobook .#wobookd .#wobook-fzf .#extension-firefox .#extension-chromium`, `result/bin/wobook --version`, `env -i result/bin/wobook-fzf` usage line, completion files present, `.xpi` contains `manifest.json`.

## 3. Home-manager module

- [ ] 3.1 Write `nix/hm-module.nix` with all `programs.wobook` options from design D4 (types, defaults, descriptions) and export it as `homeManagerModules.wobook`.
- [ ] 3.2 Implement effects: `home.packages`, `systemd.user.services.wobookd` (ExecStart, extraArgs, Restart, RestartSec, Environment, WantedBy default.target), `home.sessionVariables` for `WOBOOK_DATA_DIR`/`WOBOOK_DEVICE_NAME`, `xdg.configFile."wobook/hooks/<name>"` executable from text or path, `shellCompletions.enable = false` installing a `bin/`-only `symlinkJoin`.
- [ ] 3.3 Implement browser manifests per design D5: `dev.wochap.wobook` JSON with `path` to the native host (check `extension/` docs from `browser-extension` for whether `wobook` detects native-host mode itself or needs a `wobook native-host` wrapper script), Firefox `allowed_extensions`, Chromium `allowed_origins`, install paths for Firefox, Google Chrome, Brave and `extraChromiumDirs`.
- [ ] 3.4 Add assertions: `fzf.enable -> enable`, non-empty `chromiumExtensionIds` when any Chromium-family browser or extra dir is enabled.
- [ ] 3.5 Write `contrib/nix-config/wobook/default.nix` in the user's `_custom.programs.wobook` / `_custom.hm` shape with header comments (flake input line, `modules/shared/default.nix` import, disabling buku per host, `tui-bookmarks.sh` swap, alternative `home-manager.sharedModules` import); confirm `nix-instantiate --parse` succeeds.

## 4. Checks

- [ ] 4.1 Add `checks.wobook-fmt`, `checks.wobook-clippy` (`--all-targets -- -D warnings`) and `checks.wobook-test` via crane sharing `cargoArtifacts`; set `XDG_RUNTIME_DIR=$TMPDIR` and any env the e2e tests need to run without network.
- [ ] 4.2 Add `checks.hm-module-eval` building the activation package of a minimal `homeManagerConfiguration` that imports the module with `enable`, `fzf.enable`, `deviceName`, an inline hook and all three browsers enabled; assert the output contains `wobookd.service`, the hook and the three manifests.
- [ ] 4.3 Run `nix flake check` until it exits 0; fix clippy, fmt, sandbox or module issues found.

## 5. Documentation

- [ ] 5.1 README: "Install with Nix" section (flake input, `homeManagerModules.wobook`, option table, overlay, `nix run`, extension packages and how to load the unsigned xpi / unpacked dir).
- [ ] 5.2 README: "Migrate from buku" section following design's Migration Plan (input, enable on one host, `wobook import ~/Sync/.config/buku/bookmarks.db`, swap `buku-fzf` to `wobook-fzf` in the kitty launcher, disable buku per host, pair, repeat), plus the `DBUS_SESSION_BUS_ADDRESS` note for `notify-send` hooks.
- [ ] 5.3 manual: install the module on one real host, confirm `systemctl --user status wobookd` is active, the keybinding opens `wobook-fzf`, and Firefox/Chrome/Brave connect to the native host.
