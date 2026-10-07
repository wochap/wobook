## Context

`core-and-daemon` ships `wobookd` listening on a private unix socket with a JSON-lines protocol (`wobook_core::protocol`: `add`, `get`, `search`, `tags`, error codes `exists`, `not_found`, `hook_rejected`, `invalid_url`). WebExtensions cannot open unix sockets; the only sanctioned bridge is native messaging, where the browser spawns a host executable declared in a per-browser manifest file and exchanges length-prefixed JSON on stdio.

The user runs Firefox, google-chrome and brave on NixOS. Firefox reads host manifests from `~/.mozilla/native-messaging-hosts/<name>.json` (`allowed_extensions` by gecko id). Chromium browsers read `~/.config/google-chrome/NativeMessagingHosts/<name>.json` and `~/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts/<name>.json` (`allowed_origins` of `chrome-extension://<id>/`). Native messaging frames are a 4-byte native-endian (little-endian on x86-64 and aarch64) length followed by UTF-8 JSON; the browser caps host-to-browser messages at 1 MiB and browser-to-host at 4 GiB.

Visual language comes from the Android handoff (`design/project/wobook-tokens.css`, `readme.md`): Catppuccin Mocha/Latte, Inter + JetBrains Mono, accent only as lines and highlights, chip tag editor committing on comma or Enter with spaces allowed.

## Goals / Non-Goals

**Goals:**
- Save the active tab with tags in under 5 seconds, including from the keyboard.
- Fuzzy search bookmarks from the popup and open or copy a hit.
- Show whether the current page is already saved.
- One codebase, three browsers, no network port, no background fetch (the page is already open).
- Same error vocabulary as the CLI.

**Non-Goals:**
- Installing the host manifests (nix-module change).
- Store publication (AMO, Chrome Web Store); sideload or `web-ext run` only.
- Editing or deleting bookmarks from the popup (open the CLI or Android app).
- Omnibox keyword search, context menus, bookmark bar sync, Safari, mobile browsers.
- Reading page content beyond title and meta description.

## Decisions

### D1. Layout and build

```
extension/
├── package.json             pnpm, scripts: build, watch, test, lint, package:firefox (web-ext build)
├── tsconfig.json
├── esbuild.mjs              bundles src/background.ts and src/popup/popup.ts to dist/, copies static files
├── manifest.json            MV3, shared by both targets (see D2)
├── src/
│   ├── browser.ts           `export const api = globalThis.browser ?? globalThis.chrome` plus promise wrappers
│   ├── protocol.ts          TypeScript mirror of wobook_core::protocol request/response types
│   ├── native.ts            sendNativeMessage(request) with error mapping (D4)
│   ├── background.ts        service worker: badge updates, command handler, native proxy for popup
│   └── popup/
│       ├── popup.html, popup.css (tokens copied from design/project/wobook-tokens.css)
│       ├── popup.ts         tabs: Save, Search; state machine per D5/D6
│       ├── tag-editor.ts    chip editor component (plain DOM)
│       └── highlight.ts     index-based highlighting helper
├── icons/                   16/32/48/128 PNG plus badge-ready monochrome variants
└── tests/
    ├── popup.spec.ts        Playwright, harness page mocking the native bridge
    └── harness.html         loads popup bundle with a fake `api.runtime.sendNativeMessage`
```
esbuild over plain ES modules because the protocol types and shared helpers need bundling into both the service worker and the popup; still no framework. Alternative considered: Vite with a webextension plugin — more moving parts than needed for two entry points.

### D2. Manifest V3 shared by Firefox and Chromium

```json
{
  "manifest_version": 3,
  "name": "wobook",
  "version": "0.1.0",
  "action": { "default_popup": "popup/popup.html", "default_icon": {...} },
  "background": { "service_worker": "background.js", "scripts": ["background.js"] },
  "permissions": ["nativeMessaging", "activeTab", "scripting", "tabs", "storage"],
  "commands": { "_execute_action": { "suggested_key": { "default": "Alt+Shift+B" } } },
  "browser_specific_settings": { "gecko": { "id": "wobook@wochap.dev", "strict_min_version": "121.0" } }
}
```
Firefox 121+ accepts `background.scripts` alongside `service_worker` (it ignores the worker, Chromium ignores `scripts`), so one manifest works; Chromium warns about the unknown `browser_specific_settings` key but loads. If Chrome rejects the key in a future version, `esbuild.mjs` emits `dist/firefox/` and `dist/chromium/` with the key stripped for Chromium; the task list includes that split from the start to avoid surprises. `_execute_action` opens the popup directly, so no custom command handler is needed for the shortcut.

No `host_permissions`. `activeTab` plus `scripting` are enough to run a one-shot script in the current tab when the user opens the popup. `tabs` is required to read `tab.url` on `tabs.onActivated`/`onUpdated` for the badge without a user gesture.

### D3. Native messaging host in the CLI

`wobook native-host` (hidden from the default help listing is not necessary, keep it documented). Loop:
1. Read 4 bytes little-endian length. EOF → exit 0.
2. Reject length > 4 MiB (matches the daemon request cap) with an error response and continue.
3. Read the JSON body, parse it as `protocol::Request`. Parse failure → `{"ok":false,"error":{"code":"invalid_request",...}}`.
4. Connect to the daemon socket, send one JSON line, read one JSON line, forward it verbatim as the response frame (length-prefixed). Connection refused or missing socket → `{"ok":false,"error":{"code":"daemon_unavailable","message":"wobookd is not running ..."}}` (new code emitted only by the host, documented in `protocol.rs` as a client-side code).
5. Response frames larger than 1 MiB are replaced with `{"ok":false,"error":{"code":"response_too_large"}}` because browsers kill the host otherwise; `list` without a limit can hit this, so the popup always passes `limit`.

Every incoming frame carries `"origin": "extension"` injected by the host when the request is `add` or `update` and the field is absent, so hooks see where the write came from. The host logs to stderr only (the browser collects it in its console).

`wobook native-host --print-manifest <firefox|chrome|brave> --extension-id <id> [--binary <path>]` prints:
- firefox: `{"name":"dev.wochap.wobook","description":"wobook bookmarks","path":"<abs path to wobook>","type":"stdio","allowed_extensions":["wobook@wochap.dev"]}` (the `--extension-id` defaults to `wobook@wochap.dev` for firefox).
- chrome/brave: same but `"allowed_origins":["chrome-extension://<id>/"]`.
`--binary` defaults to the running executable's absolute path. The `README` documents the three target paths; installation is left to the user or the nix module. The host binary name in the manifest is `wobook` itself with the subcommand baked into a wrapper? Native messaging manifests take a path only, no arguments, so the manifest points to a tiny shell wrapper `wobook-native-host` (installed by the nix module; `--print-manifest` prints the path the user passes or `<dir of wobook>/wobook-native-host`). The wrapper is `exec wobook native-host "$@"` and ships in `contrib/wobook-native-host`.

### D4. Error mapping in the extension

`native.ts` turns every failure into a typed `UiError`:
- `api.runtime.lastError` / rejected promise containing "Specified native messaging host not found" or "not found" → `host_missing` ("wobook native host is not installed", link to README section).
- "Native host has exited" or "Error when communicating" → `host_crashed`.
- response `error.code === "daemon_unavailable"` → `daemon_down` ("wobookd is not running. Start it: systemctl --user start wobookd").
- `hook_rejected` → shows the hook's message verbatim.
- `exists` cannot happen because the popup always sends `merge: true`; `invalid_url` shows inline on the URL field.
- anything else → `unknown` with the raw message.

The popup never retries automatically; the badge logic treats any error as "unknown" and clears the badge.

### D5. Save tab state machine

```
opening ─▶ reading tab (tabs.query active) ─▶ scraping (scripting.executeScript: document.title, meta[name=description], meta[property=og:description])
        ─▶ lookup (native get url) ─┬─ not_found ─▶ fresh form
                                     └─ ok        ─▶ already-saved form (banner "Already saved on <created date>", tags prefilled, button "Update tags")
fresh form ─ Save ─▶ add {url, title, description, tags, fetch:false, merge:true, origin:"extension"} ─▶ saved (check, auto-close after 600 ms)
already-saved ─ Update tags ─▶ update {url, tags} ─▶ saved
any ─▶ error (D4), form stays editable, Save re-enabled
```
Pages where scripting is impossible (`about:`, `chrome://`, PDFs, the store) skip scraping and use `tab.title`. Non-http(s) tab URLs disable Save with "This page cannot be bookmarked". Tag suggestions come from `tags` (fetched once per popup open, cached in `storage.session` for 60 s) sorted by count, filtered by fuzzy prefix of the current chip input; the last 20 used tags live in `storage.local` and float to the top. The tag editor commits on comma or Enter, Backspace on an empty input removes the last chip, spaces stay inside the tag. Save is also bound to Ctrl+Enter.

### D6. Search tab

Query input debounced 80 ms; sends `search {query, limit: 50}` through the background service worker (popups can call `sendNativeMessage` directly, but routing through the worker keeps one place for error mapping and lets the badge share the code). Each hit renders title (highlighted), host+path in mono (highlighted), tiny tag chips. Highlighting maps the returned haystack `indices` using the `segments` boundaries from `fuzzy-search` so only title and URL spans are highlighted. Keyboard: Up/Down move selection, Enter opens the selected URL in a new tab and closes the popup, Ctrl+C or click copies the URL and shows "Copied" for 1 s. Empty query shows the 20 most recent. The Search tab is remembered as the last active tab in `storage.local` only when the user switches to it explicitly; the popup always opens on Save when invoked by the shortcut.

### D7. Badge

Background worker listens to `tabs.onActivated` and `tabs.onUpdated` (status `complete` or `url` change). For http(s) URLs it sends `get {url}`; `ok` → `action.setBadgeText({text:"✓", tabId})` with the accent color as badge background, `not_found` or any error → clear. Results are cached in a `Map<tabId, {url, bookmarked}>` inside the worker and invalidated when the popup saves (popup posts `badge:refresh` to the worker via `runtime.sendMessage`). The worker also clears the entry on `tabs.onRemoved`. The host spawns per message (`sendNativeMessage`), which is fine at tab-switch frequency; a persistent `connectNative` port is a later optimization.

### D8. Theming

`popup.css` copies the `:root`/`[data-theme]` variable blocks from `design/project/wobook-tokens.css` and maps `data-theme` to `@media (prefers-color-scheme: light)`. Popup width 380 px, max height 560 px. Fonts fall back to system-ui and ui-monospace; no webfont download from the popup (CSP and offline).

### D9. Testing

- Rust e2e in `crates/wobook/tests/native_host.rs`: spawn a real `wobookd` in a temp dir, spawn `wobook native-host` with the socket env, write framed `ping`, `add`, `get`, `search` and assert framed responses; frame with length 5 MiB → `invalid_request`/too large response and the host keeps serving; daemon stopped → `daemon_unavailable`; EOF → exit 0; `--print-manifest` for all three browsers produces valid JSON with the expected keys.
- Playwright (`extension/tests/popup.spec.ts`) loads `tests/harness.html`, which defines a fake `chrome` object (`tabs.query`, `scripting.executeScript`, `runtime.sendNativeMessage`, `storage.*`) scripted per test, then imports the popup bundle. Covers: fresh save flow sends the right `add` payload with `fetch:false`; already-saved banner and `Update tags`; tag editor comma/Enter/space/Backspace; host-missing, daemon-down and hook-rejected messages; search highlighting and Enter/copy actions.
- Manual smoke tasks for loading the unpacked extension in Firefox (`web-ext run`), google-chrome and brave with the manifest installed by hand.

## Risks / Trade-offs

- [Chromium rejects `browser_specific_settings` in a future release] → build already emits per-target `dist/firefox` and `dist/chromium`; the key is stripped for Chromium.
- [Host manifest `path` must be absolute and stable on NixOS; profile paths change on every rebuild] → manifests point to `~/.nix-profile/bin/wobook-native-host` or the path the nix module writes; `--print-manifest --binary` lets the user choose.
- [Native host spawn per badge update adds ~20 ms process start] → acceptable; cache per tab; move to `connectNative` port if it shows.
- [`tabs` permission triggers an install warning "Read your browsing history"] → documented; without it the badge cannot work. Could be dropped later behind an option.
- [Scraped description may be junk on SPAs] → user edits before Save; `fetch:false` keeps the daemon from overwriting it.
- [Firefox requires the gecko id to match the host manifest `allowed_extensions`] → fixed id `wobook@wochap.dev` in both places, validated by the Rust `--print-manifest` test.

## Migration Plan

1. Build: `cd extension && pnpm install && pnpm build` (inside `nix develop`).
2. Install the host manifest by hand for one browser using `wobook native-host --print-manifest chrome --extension-id <id> > ~/.config/google-chrome/NativeMessagingHosts/dev.wochap.wobook.json` and `contrib/wobook-native-host` on PATH.
3. Load `dist/chromium` unpacked (or `web-ext run --source-dir dist/firefox`), press `Alt+Shift+B`, save a tab, confirm with `wobook list`.
4. Rollback: remove the extension and the manifest file; nothing in the daemon changes.

## Open Questions

- Extension id for Chromium is derived from the packing key; the dev unpacked id differs per machine. The nix module will need a committed `key` field in the Chromium manifest to pin the id, which this change prepares by documenting how to generate it (`openssl` key → `key` field → stable id).
