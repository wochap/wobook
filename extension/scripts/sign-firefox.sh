#!/usr/bin/env bash
# Signs the Firefox build on the AMO unlisted channel; output goes to dist/signed/.
set -euo pipefail
cd "$(dirname "$0")/.."

for var in WEB_EXT_API_KEY WEB_EXT_API_SECRET; do
  if [ -z "${!var:-}" ]; then
    echo "sign-firefox: $var is not set (create AMO API credentials at https://addons.mozilla.org/developers/addon/api/key/)" >&2
    exit 1
  fi
done

node esbuild.mjs
web-ext sign --channel=unlisted --source-dir dist/firefox --artifacts-dir dist/signed
