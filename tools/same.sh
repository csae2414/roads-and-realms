#!/usr/bin/env bash
# Do the game's results stay exactly the same? Runs a few soaks and compares their sync hashes
# with the recorded ones. usage: tools/same.sh record | tools/same.sh
set -uo pipefail
cd "$(dirname "$0")/.."
# (a failed build would compare the old binary: QA round 6)
cargo build -q --release -p rr-cli || { echo "BUILD FAILED"; exit 1; }
base=local/same.txt
run() {
  echo "europe       $(target/release/rr-cli soak --mission europe --gf 120000 2>&1 | grep 'sync hash')"
  echo "random192    $(target/release/rr-cli soak --size 192 --players 6 --seed 3 --gf 80000 2>&1 | grep 'sync hash')"
  echo "fareast      $(target/release/rr-cli soak --mission fareast --gf 120000 2>&1 | grep 'sync hash')"
  echo "newcomer s4  $(target/release/rr-cli soak --mission europe --seed 4 --newcomer 1 --gf 90000 2>&1 | grep 'sync hash')"
}
if [ "${1:-}" = record ]; then
  mkdir -p local; run | tee "$base"
else
  now=$(run); echo "$now"
  if diff <(echo "$now") "$base" >/dev/null; then echo "SAME"; else echo "DIFFERENT from $base"; exit 1; fi
fi
