# extension-distribution Specification

## Purpose
Lets the wobook Firefox extension be signed for self-distribution and installed permanently, and states which browsers the extension supports.

## Requirements

### Requirement: Unlisted signing command
`extension/` SHALL provide a `sign:firefox` package script that builds the Firefox bundle and signs it on the AMO `unlisted` channel using the `WEB_EXT_API_KEY` and `WEB_EXT_API_SECRET` environment variables, writing the signed `.xpi` under the git-ignored `extension/dist/` tree. Without both variables the script SHALL exit non-zero before contacting AMO and name the missing variable.

#### Scenario: Missing credentials
- **WHEN** `pnpm sign:firefox` runs with `WEB_EXT_API_KEY` unset
- **THEN** it exits non-zero, prints a message naming `WEB_EXT_API_KEY`, and makes no network request

#### Scenario: Signed artifact location
- **WHEN** `pnpm sign:firefox` succeeds with valid credentials
- **THEN** a signed `.xpi` exists under `extension/dist/signed/` and `git status` shows no new untracked files

### Requirement: Release and install documentation
`extension/README.md` SHALL document, in order: creating AMO API credentials, running `pnpm sign:firefox`, attaching the signed `.xpi` to a GitHub release tagged `extension-v<version>`, updating `nix/extension-signed.nix` with the release URL and hash, and installing permanently through Firefox policies `ExtensionSettings."wobook@wochap.dev"` with `installation_mode = "force_installed"` and an `install_url` of `file://` plus the `extension-firefox-signed` store path. It SHALL state that release Firefox ignores `xpinstall.signatures.required` and that temporary add-ons are removed on restart.

#### Scenario: Policy snippet present
- **WHEN** a reader opens the Firefox install section of `extension/README.md`
- **THEN** it contains a home-manager `programs.firefox.policies.ExtensionSettings` example for `wobook@wochap.dev` using `force_installed`

### Requirement: Mobile support boundary documented
`extension/README.md` and `README.md` SHALL state that the extension does not support mobile browsers, because Firefox for Android lacks `nativeMessaging` and Chrome for Android has no extensions, and SHALL point Android users to the wobook app's share sheet.

#### Scenario: Mobile question answered
- **WHEN** a reader searches either README for "Android"
- **THEN** they find the statement that the extension is desktop-only and that the app's share sheet saves bookmarks on Android
