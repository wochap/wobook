## 1. Native messaging host (Rust)

- [ ] 1.1 Add `daemon_unavailable`, `response_too_large` and `invalid_request` client-side error codes to `wobook_core::protocol` documentation and a helper to build error responses.
- [ ] 1.2 Implement `crates/wobook/src/native_host.rs`: framed stdin reader (4-byte LE length, 4 MiB cap, skip oversized payload), request parsing, `origin: "extension"` injection for `add`/`update`, one socket request per frame, framed stdout writer with the 1 MiB response cap, `daemon_unavailable` mapping, exit 0 on EOF and 70 on stdout failure.
- [ ] 1.3 Implement `--print-manifest <firefox|chrome|brave> [--extension-id] [--binary]` with the `dev.wochap.wobook` name, absolute path default (`<dir of current exe>/wobook-native-host`), usage error when the id is missing for chrome/brave.
- [ ] 1.4 Add `contrib/wobook-native-host` wrapper (`exec wobook native-host "$@"`), executable, shellcheck clean.
- [ ] 1.5 e2e `crates/wobook/tests/native_host.rs`: spawn `wobookd` in a temp dir, drive the host with framed `ping`/`add`/`get`/`search`, assert `origin` reaches hooks via a `post-add` fixture hook, oversized frame then ping, malformed JSON, daemon stopped → `daemon_unavailable`, EOF → exit 0, manifest JSON for all three browsers.

## 2. Extension scaffold

- [ ] 2.1 Create `extension/` with `package.json` (pnpm; scripts build/watch/test/lint/package:firefox), `tsconfig.json`, `esbuild.mjs` emitting `dist/firefox` and `dist/chromium` (Chromium copy strips `browser_specific_settings`), static copy of `manifest.json`, `popup/*.html|css`, `icons/`.
- [ ] 2.2 Write `manifest.json` per design D2 (MV3, permissions `nativeMessaging activeTab scripting tabs storage`, `_execute_action` `Alt+Shift+B`, gecko id `wobook@wochap.dev`, background worker plus scripts).
- [ ] 2.3 Write `src/browser.ts` (promise-wrapped `browser ?? chrome`), `src/protocol.ts` mirroring `wobook_core::protocol` request/response/error types, `src/native.ts` with `UiError` mapping per design D4.
- [ ] 2.4 Copy the Catppuccin token blocks from `design/project/wobook-tokens.css` into `popup/popup.css` with `prefers-color-scheme` switching, system font fallbacks, 380 px width.
- [ ] 2.5 Add nodejs, pnpm and web-ext to the flake dev shell.

## 3. Popup: Save tab

- [ ] 3.1 Implement the Save state machine from design D5: read active tab, one-shot `scripting.executeScript` scraper (title, meta description, og:description) with fallbacks for unscriptable pages, non-http(s) disables Save.
- [ ] 3.2 Implement `tag-editor.ts`: chips, commit on comma/Enter, spaces inside tags, Backspace removes last chip, lowercase/trim/dedupe, suggestions from `tags` (60 s session cache) plus 20 recent tags in `storage.local`, Tab/click accepts a suggestion.
- [ ] 3.3 Implement `get` lookup, already-saved banner with created date, prefilled tags, "Update tags" sending `update {url, tags}`; fresh path sends `add` with `fetch:false`, `merge:true`, `origin:"extension"`; `Ctrl+Enter` saves; confirmation then auto-close after 600 ms; notify the worker to refresh the badge.
- [ ] 3.4 Implement error rendering for `host_missing`, `host_crashed`, `daemon_down` (with systemctl hint), `hook_rejected` (verbatim) and `invalid_url` (inline), keeping the form editable.

## 4. Popup: Search tab and badge

- [ ] 4.1 Implement Search tab: 80 ms debounce, `search {query, limit:50}` via the worker, empty query shows 20 most recent, result rows (title, mono host+path, tiny tag chips), "Nothing matches" state, shared error states.
- [ ] 4.2 Implement `highlight.ts` mapping `indices` and segment boundaries to highlight spans on title and URL only.
- [ ] 4.3 Implement keyboard and mouse actions: Up/Down selection, Enter or open icon → `tabs.create` and close, click row or `Ctrl+C` → clipboard with a 1 s "Copied" state; remember explicit tab switch in `storage.local`, always open on Save from the shortcut.
- [ ] 4.4 Implement `background.ts`: message router for popup requests, badge logic on `tabs.onActivated`/`onUpdated` (http(s) complete only), per-tab cache, `onRemoved` cleanup, `badge:refresh` message, silent failure handling, accent badge color.

## 5. Tests and docs

- [ ] 5.1 Playwright setup in `extension/tests` with `harness.html` injecting a scriptable fake `chrome` (`tabs.query`, `scripting.executeScript`, `runtime.sendNativeMessage`, `runtime.sendMessage`, `storage.local/session`, `tabs.create`, `action.*`) and loading the popup bundle.
- [ ] 5.2 Playwright specs: fresh save payload; already-saved banner and Update tags; tag editor comma/Enter/space/Backspace/suggestion; host-missing, daemon-down, hook-rejected messages; search highlighting, Enter opens, click copies; non-http(s) page disables Save.
- [ ] 5.3 Lint and typecheck (`tsc --noEmit`, eslint or biome) wired into `pnpm lint`; `pnpm build` produces both dist folders; `web-ext lint` passes on `dist/firefox`.
- [ ] 5.4 Write `extension/README.md`: build, load unpacked per browser, host manifest paths for Firefox/google-chrome/brave, `--print-manifest` usage, pinning the Chromium extension id with a `key` field, troubleshooting the three error states.
- [ ] 5.5 Manual smoke: install the manifest and load the extension in Firefox via `web-ext run`, save a tab, verify with `wobook list`.
- [ ] 5.6 Manual smoke: same in google-chrome and brave with the unpacked `dist/chromium`, confirm the badge and `Alt+Shift+B`.
