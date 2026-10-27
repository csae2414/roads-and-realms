#!/usr/bin/env bash
# UI regressions the smoke test doesn't reach (QA round 4, #2): a keyboard cursor kept over a
# load used to crash the game on Enter when the loaded map is smaller.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -q --release -p rr-app
out=local/ui_regress; rm -rf "$out"; mkdir -p "$out/data"
export XDG_DATA_HOME_TEST="$(realpath "$out/data")"
SCRIPT="sleep 4; key F5; sleep 2" tools/uitest.sh "$out/small" --size 48 >/dev/null 2>&1 || true
SCRIPT="sleep 5; mousemove 640 400; key Return; sleep 1; key F9; sleep 2; key Return; sleep 2" \
  tools/uitest.sh "$out/load" --size 256 --players 2 --seed 3 >/dev/null 2>&1 || true
if grep -q panicked "$out/load/log.txt"; then
  echo "UI regress FAILED"; grep -A2 panicked "$out/load/log.txt"; exit 1
fi
echo "UI regress OK"
