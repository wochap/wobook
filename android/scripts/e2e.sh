#!/usr/bin/env bash
# End-to-end: debug APK on the headless emulator against a desktop wobookd
# fixture reachable from the emulator as 10.0.2.2. Run inside
# `nix develop .#android-emulator`:
#   android/scripts/e2e.sh            all flows
#   android/scripts/e2e.sh --keep     leave the emulator running afterwards
set -Eeuo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ANDROID_DIR=$(dirname "$HERE")
ROOT=$(dirname "$ANDROID_DIR")
FLOWS=$ANDROID_DIR/maestro
APP=dev.wochap.wobook
PORT=${WOBOOK_E2E_PORT:-47391}
KEEP=0
[[ ${1-} == --keep ]] && KEEP=1

die() { echo "e2e.sh: $*" >&2; exit 1; }
log() { echo "e2e.sh: $*" >&2; }
command -v maestro >/dev/null || die "maestro not found; use the android-emulator devShell"

WORK=$(mktemp -d -t wobook-e2e.XXXXXX)
DAEMON_PID=
PAIR_PID=
cleanup() {
  local status=$?
  [[ -n $PAIR_PID ]] && kill "$PAIR_PID" 2>/dev/null || true
  if [[ -n $DAEMON_PID ]]; then
    kill "$DAEMON_PID" 2>/dev/null || true
    wait "$DAEMON_PID" 2>/dev/null || true
  fi
  ((KEEP)) || "$HERE/emulator.sh" stop || true
  if ((status != 0)); then
    log "failed; fixture log:"
    tail -40 "$WORK/wobookd.log" >&2 || true
  fi
  rm -rf "$WORK"
  exit "$status"
}
trap cleanup EXIT

log "building APK and desktop fixture"
(cd "$ANDROID_DIR" && ./gradlew --quiet assembleDebug)
(cd "$ROOT" && cargo build --quiet -p wobookd -p wobook)
WOBOOKD=$ROOT/target/debug/wobookd
WOBOOK=$ROOT/target/debug/wobook

"$HERE/emulator.sh" start
adb -e install -r "$ANDROID_DIR/app/build/outputs/apk/debug/app-debug.apk" >/dev/null
adb -e shell pm clear "$APP" >/dev/null

log "starting wobookd fixture on 0.0.0.0:$PORT (advertised as 10.0.2.2:$PORT)"
export WOBOOK_SOCKET=$WORK/wobookd.sock
WOBOOK_SYNC_PORT=$PORT WOBOOK_SYNC_ADVERTISE=10.0.2.2:$PORT WOBOOK_DISCOVERY=off \
  WOBOOK_DEVICE_NAME=fixture-desktop \
  "$WOBOOKD" --data-dir "$WORK/data" --socket "$WOBOOK_SOCKET" --hooks-dir "$WORK/hooks" \
  >"$WORK/wobookd.log" 2>&1 &
DAEMON_PID=$!
for _ in $(seq 50); do [[ -S $WOBOOK_SOCKET ]] && break; sleep 0.2; done
[[ -S $WOBOOK_SOCKET ]] || die "wobookd did not start"
"$WOBOOK" add https://ui.shadcn.com/ --title "shadcn/ui — Build your component library" --tags "react,ui library,tailwind" --no-fetch >/dev/null
"$WOBOOK" add https://wiki.archlinux.org/title/Systemd/Timers --title "Arch Wiki — systemd/Timers" --tags "linux,systemd,docs" --no-fetch >/dev/null
"$WOBOOK" add https://github.com/jarun/buku --title "buku — bookmark manager like a text-based mind" --tags "linux,cli" --no-fetch >/dev/null

flow() { log "flow $1"; maestro --device "$(adb -e get-serialno)" test "${@:2}" "$FLOWS/$1.yaml"; }
share() {
  adb -e shell am start -a android.intent.action.SEND -t text/plain \
    --es android.intent.extra.TEXT "'$1'" --es android.intent.extra.SUBJECT "'$2'" \
    -n "$APP/.ShareReceiverActivity" >/dev/null
}

flow onboarding

share https://react.dev/learn "Quick Start – React"
flow share-add

log "pairing: fixture offers, phone pastes"
"$WOBOOK" pair --json --yes >"$WORK/pair.out" 2>"$WORK/pair.err" &
PAIR_PID=$!
for _ in $(seq 50); do [[ -s $WORK/pair.out ]] && break; sleep 0.2; done
PAYLOAD=$(head -1 "$WORK/pair.out")
[[ -n $PAYLOAD ]] || die "no pairing payload"
flow pair-paste -e "PAYLOAD=$(jq -rn --arg p "$PAYLOAD" '$p|@uri')"
wait "$PAIR_PID" || die "fixture side of pairing failed: $(cat "$WORK/pair.err")"
PAIR_PID=

flow search-open
flow edit-tags
flow delete-undo
flow devices
flow settings-icons

log "sync: desktop → phone"
"$WOBOOK" add https://conv.example/ --title "Convergence check" --tags sync --no-fetch >/dev/null
flow sync-converge

log "sync: phone → desktop"
share https://phone.example/ "Saved on the phone"
flow share-save
for _ in $(seq 30); do
  "$WOBOOK" show https://phone.example/ >/dev/null 2>&1 && break
  sleep 1
done
"$WOBOOK" show https://phone.example/ >/dev/null || die "phone bookmark did not reach the fixture within 30 s"

# Runs last: it wipes the phone and adds a second phone identity to the
# fixture's trust store, which would break the device list in devices.yaml.
log "pairing during onboarding: fresh phone, Back from Devices lands on Home"
"$WOBOOK" pair --json --yes >"$WORK/pair2.out" 2>"$WORK/pair2.err" &
PAIR_PID=$!
for _ in $(seq 50); do [[ -s $WORK/pair2.out ]] && break; sleep 0.2; done
PAYLOAD=$(head -1 "$WORK/pair2.out")
[[ -n $PAYLOAD ]] || die "no pairing payload for onboarding-pair"
flow onboarding-pair -e "PAYLOAD=$(jq -rn --arg p "$PAYLOAD" '$p|@uri')"
wait "$PAIR_PID" || die "fixture side of onboarding pairing failed: $(cat "$WORK/pair2.err")"
PAIR_PID=

log "all flows passed"
