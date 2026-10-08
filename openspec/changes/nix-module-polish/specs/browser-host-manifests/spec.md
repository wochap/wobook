## MODIFIED Requirements

### Requirement: Manifest content
The module SHALL generate a native messaging host manifest named `dev.wochap.wobook` with `type = "stdio"`, `path` pointing at the native host executable from the installed `wobook` package, `allowed_extensions = [ "wobook@wochap.dev" ]` (the gecko id in `extension/manifest.json`) for Firefox-family browsers and `allowed_origins = [ "chrome-extension://<id>/" ... ]` from `browsers.chromiumExtensionIds` for Chromium-family browsers.

#### Scenario: Firefox manifest
- **WHEN** `browsers.firefox.enable = true`
- **THEN** `~/.mozilla/native-messaging-hosts/dev.wochap.wobook.json` exists with `allowed_extensions` equal to `["wobook@wochap.dev"]` and `path` under `/nix/store`

#### Scenario: Chromium manifest origins
- **WHEN** `chromiumExtensionIds = [ "abc" "def" ]` and `googleChrome.enable = true`
- **THEN** the Chrome manifest's `allowed_origins` is `["chrome-extension://abc/", "chrome-extension://def/"]` and has no `allowed_extensions` key

### Requirement: Per-browser install paths
Manifests SHALL be installed only for enabled browsers at: Firefox `~/.mozilla/native-messaging-hosts/`, Google Chrome `~/.config/google-chrome/NativeMessagingHosts/`, Brave `~/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts/`, plus `~/.config/<dir>/NativeMessagingHosts/` for each entry of `browsers.extraChromiumDirs` and `~/<dir>/native-messaging-hosts/` (Firefox manifest) for each entry of `browsers.extraFirefoxDirs`. `extraFirefoxDirs` SHALL default to `[]` and SHALL NOT require `firefox.enable`.

#### Scenario: Only Brave enabled
- **WHEN** `brave.enable = true` and the other browsers are false
- **THEN** only the Brave path receives a manifest

#### Scenario: Extra Chromium dir
- **WHEN** `extraChromiumDirs = [ "chromium" ]`
- **THEN** `~/.config/chromium/NativeMessagingHosts/dev.wochap.wobook.json` is installed with the Chromium-style manifest

#### Scenario: Extra Firefox dir
- **WHEN** `extraFirefoxDirs = [ ".librewolf" ]`
- **THEN** `~/.librewolf/native-messaging-hosts/dev.wochap.wobook.json` is installed with the Firefox-style manifest
