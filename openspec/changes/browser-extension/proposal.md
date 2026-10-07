## Why

Saving a bookmark from the desktop browser today means copying the URL, switching to a terminal and running the fzf wrapper. buku never had a browser integration either. A WebExtension that talks to the running `wobookd` through native messaging makes "save this tab with tags" a two-second action and gives in-browser fuzzy search, without opening any network port on the desktop.

## What Changes

- New `extension/` directory: one WebExtension (Manifest V3) codebase for Firefox, google-chrome and brave, TypeScript bundled with esbuild, no UI framework, Catppuccin tokens copied from `design/project/wobook-tokens.css`.
- New `wobook native-host` subcommand on the existing CLI: a native messaging host (`dev.wochap.wobook`) that reads length-prefixed JSON frames on stdio and proxies each one to the daemon unix socket as a single request. `--print-manifest <firefox|chrome|brave> --extension-id <id>` prints the host manifest for manual installation; the nix-module change installs it.
- Popup "Save" tab: current tab URL, title and page meta description prefilled via `scripting.executeScript`, chip tag editor with autocomplete from daemon `tags`, Save = `add` with `merge: true` and `fetch: false`; already-saved URLs show "Already saved on <date>" with existing tags and Save becomes "Update tags".
- Popup "Search" tab: fuzzy search through daemon `search`, matched characters highlighted from returned indices, Enter opens in a new tab, click copies the URL.
- Toolbar badge: a check mark when the current tab's URL is already bookmarked, refreshed on tab change and cached per tab.
- Keyboard shortcut `Alt+Shift+B` opens the popup.
- Explicit error states for daemon not running, native host not installed, and `hook_rejected`.
- Dev shell additions: nodejs, pnpm, web-ext.

## Capabilities

### New Capabilities
- `native-messaging-host`: stdio framing, proxying to the daemon socket, error mapping, manifest printing for the three browsers.
- `extension-save-popup`: prefill from the active tab, tag editor, save/merge semantics, already-saved state, error states, shortcut.
- `extension-search`: fuzzy search UI, highlighting, open and copy actions.
- `extension-badge`: bookmarked indicator on the toolbar icon.

### Modified Capabilities
- `cli`: ADDED requirement for the `native-host` subcommand and manifest printing (delta only; no existing requirement changes).

## Impact

- New code in `extension/` and a new module in `crates/wobook`; `wobook_core::protocol` is reused unchanged, so the daemon is untouched.
- Browser-side dependencies: TypeScript, esbuild, `@types/webextension-polyfill` or hand-written typings, Playwright for popup tests, web-ext for Firefox packaging.
- Host manifests must be installed into per-browser directories; this change documents the paths and ships the printer, the nix-module change writes the files.
- Depends on `core-and-daemon` (daemon socket, `add`/`get`/`search`/`tags` commands, `hook_rejected` error code).
