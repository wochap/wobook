# browser-host-manifests Specification

## Purpose
Native messaging host manifest generation and installation paths for Firefox, Google Chrome and Brave.

## Requirements

### Requirement: Manifest content
The module SHALL generate a native messaging host manifest named `dev.wochap.wobook` with `type = "stdio"`, `path` pointing at the native host executable from the installed `wobook` package, `allowed_extensions = [ browsers.firefoxExtensionId ]` for Firefox and `allowed_origins = [ "chrome-extension://<id>/" ... ]` from `browsers.chromiumExtensionIds` for Chromium-family browsers.

#### Scenario: Firefox manifest
- **WHEN** `browsers.firefox.enable = true` and `firefoxExtensionId = "wobook@wochap.dev"`
- **THEN** `~/.mozilla/native-messaging-hosts/dev.wochap.wobook.json` exists with `allowed_extensions` equal to `["wobook@wochap.dev"]` and `path` under `/nix/store`

#### Scenario: Chromium manifest origins
- **WHEN** `chromiumExtensionIds = [ "abc" "def" ]` and `googleChrome.enable = true`
- **THEN** the Chrome manifest's `allowed_origins` is `["chrome-extension://abc/", "chrome-extension://def/"]` and has no `allowed_extensions` key

### Requirement: Per-browser install paths
Manifests SHALL be installed only for enabled browsers at: Firefox `~/.mozilla/native-messaging-hosts/`, Google Chrome `~/.config/google-chrome/NativeMessagingHosts/`, Brave `~/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts/`, plus `~/.config/<dir>/NativeMessagingHosts/` for each entry of `browsers.extraChromiumDirs`.

#### Scenario: Only Brave enabled
- **WHEN** `brave.enable = true` and the other browsers are false
- **THEN** only the Brave path receives a manifest

#### Scenario: Extra Chromium dir
- **WHEN** `extraChromiumDirs = [ "chromium" ]`
- **THEN** `~/.config/chromium/NativeMessagingHosts/dev.wochap.wobook.json` is installed with the Chromium-style manifest

### Requirement: Chromium ids required
Enabling any Chromium-family browser with an empty `chromiumExtensionIds` SHALL fail evaluation with an assertion.

#### Scenario: Missing id
- **WHEN** `googleChrome.enable = true` and `chromiumExtensionIds = []`
- **THEN** evaluation fails with an assertion mentioning `chromiumExtensionIds`

### Requirement: Host path survives package upgrades
The manifest `path` SHALL reference the exact store path of the configured `package` (or its wrapper), so a rebuild that changes the package rewrites the manifest.

#### Scenario: Package override
- **WHEN** `programs.wobook.package` is overridden to a different derivation
- **THEN** the generated manifests point at the overridden store path
