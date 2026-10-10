# wobook browser extension

One Manifest V3 WebExtension for Firefox, google-chrome and brave. It talks to
the running `wobookd` through the `wobook native-host` native messaging host;
no network port is opened.

- **Save** tab: current tab URL, title and meta description, chip tag editor
  (comma or Enter commits, spaces allowed), `Ctrl+Enter` saves. Already-saved
  pages show "Already saved on <date>" and the button becomes "Update tags".
- **Search** tab: fuzzy search with highlighted matches. Up/Down select, Enter
  or the ↗ icon opens in a new tab, click or `Ctrl+C` copies the URL.
- Toolbar badge ✓ when the current tab is bookmarked.
- `Alt+Shift+B` opens the popup (change it in the browser's shortcut settings).

## Build

Inside `nix develop` (provides nodejs, pnpm, web-ext):

```sh
cd extension
pnpm install
pnpm build          # dist/firefox and dist/chromium
pnpm lint           # tsc, biome, web-ext lint
pnpm test           # Playwright popup tests (headless Chromium)
pnpm package:firefox
```

`dist/chromium` is the same build without `browser_specific_settings`.

## Load the extension

- **Firefox**: `web-ext run --source-dir dist/firefox`, or `about:debugging` →
  This Firefox → Load Temporary Add-on → `dist/firefox/manifest.json`. The
  gecko id is fixed to `wobook@wochap.dev`.
- **google-chrome / brave**: `chrome://extensions` (or `brave://extensions`) →
  Developer mode → Load unpacked → `dist/chromium`. Copy the extension id shown
  on the card; the host manifest needs it.

## Permanent install (Firefox)

Release Firefox only keeps signed extensions: it ignores
`xpinstall.signatures.required`, and temporary add-ons from `about:debugging`
are removed on restart. Sign the extension on the AMO "unlisted" channel
(self-distribution, no public listing) and install it through Firefox policies.

1. Create AMO API credentials at
   <https://addons.mozilla.org/developers/addon/api/key/> and export them:
   `export WEB_EXT_API_KEY=user:... WEB_EXT_API_SECRET=...`. Never commit them.
2. Run `pnpm lint` so AMO's automated validation has nothing to flag.
3. Bump `version` in `manifest.json` (and `package.json`). AMO rejects a
   version it has already signed.
4. Run `pnpm sign:firefox`. The signed `.xpi` lands in `dist/signed/`.
5. Create a GitHub release tagged `extension-v<version>` and attach the signed
   file as `wobook-<version>.xpi`.
6. Update `nix/extension-signed.nix` in the same commit as the version bump:
   set `version`, `url`, and `hash` from

   ```sh
   nix store prefetch-file --json https://github.com/wochap/wobook/releases/download/extension-v<version>/wobook-<version>.xpi
   ```

   While `hash = ""` the flake has no `extension-firefox-signed` package.
7. Install permanently with home-manager:

   ```nix
   programs.firefox.policies.ExtensionSettings."wobook@wochap.dev" = {
     installation_mode = "force_installed";
     install_url = "file://${inputs.wobook.packages.${pkgs.system}.extension-firefox-signed}";
   };
   ```

## Mobile

There is no mobile browser extension. Firefox for Android lacks
`nativeMessaging`, and Chrome for Android has no extensions. On Android, save
bookmarks through the wobook app's share sheet.

## Native host

Native messaging manifests take a path without arguments, so the manifest
points to `contrib/wobook-native-host` (`exec wobook native-host "$@"`). Put it
on a stable path, e.g. `~/.local/bin/wobook-native-host`, next to `wobook` on
`PATH`.

Print a manifest with `wobook native-host --print-manifest <browser>`:

```sh
# Firefox (extension id defaults to wobook@wochap.dev)
wobook native-host --print-manifest firefox --binary ~/.local/bin/wobook-native-host \
  > ~/.mozilla/native-messaging-hosts/dev.wochap.wobook.json

# google-chrome
wobook native-host --print-manifest chrome --extension-id <id> --binary ~/.local/bin/wobook-native-host \
  > ~/.config/google-chrome/NativeMessagingHosts/dev.wochap.wobook.json

# brave
wobook native-host --print-manifest brave --extension-id <id> --binary ~/.local/bin/wobook-native-host \
  > ~/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts/dev.wochap.wobook.json
```

Without `--binary` the path defaults to `wobook-native-host` next to the
running `wobook` executable. On NixOS avoid store paths that change on every
rebuild; prefer `~/.nix-profile/bin/wobook-native-host`.

### Pinning the Chromium extension id

Unpacked extensions get an id derived from their directory, so it differs per
machine. To pin it, add a public key to `manifest.json` as `key`:

```sh
openssl genrsa 2048 > key.pem
openssl rsa -in key.pem -pubout -outform DER | base64 -w0   # value of "key"
openssl rsa -in key.pem -pubout -outform DER | sha256sum | head -c32 | tr 0-9a-f a-p   # the id
```

Keep `key.pem` private; only the base64 public key goes into the manifest.

## Troubleshooting

- **"wobook native host is not installed"**: the manifest file is missing, in
  the wrong directory, has a non-absolute `path`, or `allowed_origins` /
  `allowed_extensions` does not match the extension id. Re-run
  `--print-manifest` with the right id.
- **"wobook native host crashed"**: the wrapper or `wobook` is not executable
  or not on the `PATH` the browser sees. Run the wrapper by hand: it should wait
  on stdin and exit 0 on Ctrl+D. Host logs go to the browser console (Chromium:
  launch from a terminal; Firefox: Browser Console).
- **"An unexpected error occurred"** (Firefox): the host exited without
  replying, so the popup reports it as a crash. Run the wrapper by hand with the
  arguments the browser passes, e.g.
  `wobook-native-host ~/.mozilla/native-messaging-hosts/dev.wochap.wobook.json wobook@wochap.dev`,
  and check that it waits on stdin instead of printing a usage error.
- **"wobookd is not running"**: start it with `systemctl --user start wobookd`.

The popup always opens on the Save tab; the shortcut cannot be told apart from
a toolbar click. An explicit switch to Search is recorded in `storage.local`
(`lastTab`).
