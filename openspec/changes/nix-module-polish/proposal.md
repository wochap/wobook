## Why

Overlay and `packages.wobook` users cannot opt out of shell completions; only the home-manager module can. The Firefox extension id is fixed in `extension/manifest.json`, so `browsers.firefoxExtensionId` is a knob that can only break things. Firefox forks (LibreWolf) have no manifest path. The README does not explain that the module never installs the extension itself.

## What Changes

- `nix/package.nix` takes `withShellCompletions ? true`; `false` skips zsh/fish/bash completion install. The native-host wrapper is always installed.
- **BREAKING** remove `programs.wobook.browsers.firefoxExtensionId`; the Firefox manifest's `allowed_extensions` is hardcoded to `[ "wobook@wochap.dev" ]`.
- Add `programs.wobook.browsers.extraFirefoxDirs` (list of home-relative dirs, default `[]`, example `[ ".librewolf" ]`); each gets `<dir>/native-messaging-hosts/dev.wochap.wobook.json` with the Firefox manifest.
- README: options table fixed (no `firefoxExtensionId`, add `extraFirefoxDirs`, browser `*.enable` only writes the host manifest); new "Installing the extension" guide (Firefox, Chrome/Brave, forks); short shell completions section (fpath, `eval`, `zsh-defer` after `compinit`; `withShellCompletions`).
- `nix/checks.nix`: HM eval exercises `extraFirefoxDirs`; a package check builds `withShellCompletions = false` and asserts no `share/`.

## Capabilities

### New Capabilities

### Modified Capabilities
- `nix-packages`: completions become optional via `withShellCompletions`.
- `home-manager-module`: option list drops `firefoxExtensionId`, adds `extraFirefoxDirs`.
- `browser-host-manifests`: fixed Firefox id; fork install paths.

## Impact

`nix/package.nix`, `nix/hm-module.nix`, `nix/checks.nix`, `README.md`, the three specs. Users setting `firefoxExtensionId` must delete it.
