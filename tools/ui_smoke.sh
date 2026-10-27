#!/usr/bin/env bash
# UI smoke test: menu -> Europe -> place a building (during the First steps the window opens on
# the tab of the next step's building, item 588: the woodcutter on Small) -> road to the HQ flag.
# Checks the command log.
set -euo pipefail
cd "$(dirname "$0")/.."
# always the current code (the release binary can be older than the sources)
cargo build -q --release -p rr-app
out=local/ui_smoke; rm -rf "$out"
RR_LOG=1 LANG=en_US.UTF-8 SCRIPT="sleep 1; mousemove 468 220; click 1; sleep 2; mousemove 430 613; click 1; sleep 0.5; mousemove 752 320; click 1; sleep 0.8; mousemove 850 440; click 1; sleep 0.5; mousemove 670 392; click 1; sleep 3; shot" tools/uitest.sh "$out" --menu --auto-road
grep -q "cmd PlaceBuilding" "$out/log.txt" && grep -q "cmd BuildRoad" "$out/log.txt" && ! grep -q "CommandRejected" "$out/log.txt" && echo "UI smoke OK" || { echo "UI smoke FAILED"; cat "$out/log.txt"; exit 1; }
