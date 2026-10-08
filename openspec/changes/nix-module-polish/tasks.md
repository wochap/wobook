## 1. Package

- [ ] 1.1 `nix/package.nix`: add `withShellCompletions ? true`; wrap completion install in `lib.optionalString`, keep native-host wrapper unconditional
- [ ] 1.2 `nix/checks.nix`: `wobook-no-completions` check calling `package.nix` with `withShellCompletions = false`, asserting no `share/` and both binaries present

## 2. Home-manager module

- [ ] 2.1 `nix/hm-module.nix`: remove `browsers.firefoxExtensionId`; hardcode `allowed_extensions = [ "wobook@wochap.dev" ]`
- [ ] 2.2 Add `browsers.extraFirefoxDirs` (listOf str, default `[]`, example `[ ".librewolf" ]`, description says home-relative vs `extraChromiumDirs` `~/.config`-relative); write `<dir>/native-messaging-hosts/dev.wochap.wobook.json`
- [ ] 2.3 Add "relative to ~/.config" wording check on `extraChromiumDirs` description
- [ ] 2.4 `nix/checks.nix` HM eval: set `extraFirefoxDirs = [ ".librewolf" ]`, assert file exists and contains `wobook@wochap.dev`

## 3. README

- [ ] 3.1 Options table: drop `firefoxExtensionId`, add `extraFirefoxDirs`, state browser `*.enable` options only write the host manifest
- [ ] 3.2 "Installing the extension" guide: module does not install the extension (writes manifest, installs `wobook` + `wobook-native-host`, `wobookd` must run); Firefox (`nix build github:wochap/wobook#extension-firefox -o ff-ext`, about:debugging Load Temporary Add-on `ff-ext/wobook-<version>.xpi`, temporary, Developer Edition/Nightly with `xpinstall.signatures.required = false` for permanent, fixed id `wobook@wochap.dev`); Chrome/Brave (`nix build github:wochap/wobook#extension-chromium -o chromium-ext`, `cp -rL` + `chmod -R u+w` to stable path since id derives from load path, Developer mode Load unpacked, copy id to `chromiumExtensionIds`, rebuild HM, restart browser; list because each Chromium-family browser generates its own id); forks via `extraFirefoxDirs` / `extraChromiumDirs`
- [ ] 3.3 Shell completions section: fpath, `eval "$(wobook completions zsh)"`, `zsh-defer eval "$(wobook completions zsh)"` (after `compinit`), HM `shellCompletions.enable`, package `withShellCompletions`

## 4. Verify

- [ ] 4.1 `rg firefoxExtensionId` returns nothing outside archived changes
- [ ] 4.2 `nix flake check` passes
