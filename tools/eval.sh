#!/usr/bin/env bash
# The periodic eval: checks, a long fuzz run, the UI smoke test, soaks on every campaign
# region and the AI ladder. Prints a short summary; exit code 1 if anything failed.
set -uo pipefail
cd "$(dirname "$0")/.."
fail=0
echo "== check"; tools/check.sh >/tmp/rr-eval-check.log 2>&1 && echo ok || { echo FAILED; fail=1; }
cargo test --release 2>/dev/null | grep -E "^test result" | awk '{s+=$4; f+=$6} END {print "tests passed", s, "failed", f}'
echo "== long AI games (ignored in check.sh)"; long=$(cargo test -q --release -p rr-ai -- --ignored 2>&1 | grep -E "test result|panicked" | grep -v " 0 passed; 0 failed" | head -3); echo "$long"; grep -q "panicked\|FAILED" <<<"$long" && fail=1
# (QA round 6, #2: the fuzz, the hashes and the newcomer lines below could not fail the eval)
echo "== fuzz"; fz=$(RR_FUZZ_SEEDS=${FUZZ_SEEDS:-16} RR_FUZZ_FRAMES=40000 RR_FUZZ_RATE=30 cargo test -q --release -p rr-ai --test fuzz 2>&1 | grep -E "test result|panicked" | head -2); echo "$fz"
grep -q "panicked\|FAILED" <<<"$fz" && fail=1; grep -q "test result: ok" <<<"$fz" || fail=1
# (and the README's busy one: a player removing roads and flags at every turn; QA round 9, #1 hid
# from the rate-30 run)
fz=$(RR_FUZZ_SEEDS=30 RR_FUZZ_RATE=100 cargo test -q --release -p rr-ai --test fuzz 2>&1 | grep -E "test result|panicked" | head -2); echo "busy: $fz"
grep -q "panicked\|FAILED" <<<"$fz" && fail=1; grep -q "test result: ok" <<<"$fz" || fail=1
# (no tee to /dev/stderr: with both streams in one file it overwrote the lines above)
echo "== ui smoke"; smoke=$(tools/ui_smoke.sh 2>&1 | tail -1); echo "$smoke"; grep -q OK <<<"$smoke" || fail=1
echo "== ui regress"; reg=$(tools/ui_regress.sh 2>&1 | tail -1); echo "$reg"; grep -q OK <<<"$reg" || fail=1
echo "== same results as recorded (sync hashes)"; tools/same.sh >/tmp/rr-eval-same.log 2>&1; same=$?; tail -1 /tmp/rr-eval-same.log
[ $same -eq 0 ] && grep -q "^SAME" /tmp/rr-eval-same.log || { echo "  FAILED (re-record with tools/same.sh record after a version bump)"; fail=1; }
echo "== loudness"; uv run -q --no-project --with numpy python tools/loudness.py --check >/tmp/rr-eval-loud.log 2>&1 && echo ok || { echo "off target (tools/loudness.py levels them)"; fail=1; }
cargo build -q --release -p rr-cli
for m in europe mediterranean northamerica fareast north; do
  echo "== soak $m"
  line=$(target/release/rr-cli soak --mission "$m" --gf "${SOAK_GF:-120000}" --audit 1 2>&1 \
    | grep -oE '"Attack": [0-9]+|"ev:captured": [0-9]+|player [0-9] defeated|frozen [1-9][0-9]*|panicked.*|audit failed.*' | sort -u | paste -sd' ')
  echo "$line"
  # a broken soak fails the eval (QA round 5, #1: an audit failure read as green)
  if echo "$line" | grep -qE 'frozen|panicked|audit failed'; then echo "  FAILED"; fail=1; fi
done
# the app's Europe at each level (architect round 3, #4: until item 551 the stand-in met five
# allied rivals, a harder game than the players'; rr-cli prints the cast it plays)
echo "== newcomer (easy stand-in, keeps the truce; the app's easy Europe: two rivals at war, an hour of truce; 90 min; a defeat fails)"
# three seeds: one decides too much by chance (new-player round 8, #7)
# to minute 90, and a defeat by easy rivals fails the eval (new-player round 9, #8: the line
# could not fail, and stopped before the second hour)
for sd in 1 2 3; do
  line=$(target/release/rr-cli soak --mission europe --seed $sd --newcomer 1 --gf 108000 2>&1 | grep -E "newcomer at 90|player 0 defeated" | head -1)
  echo "seed $sd: $line"
  if echo "$line" | grep -q "defeated\|castle LOST" || ! echo "$line" | grep -q "newcomer at 90"; then echo "  FAILED"; fail=1; fi
done
for lv in normal hard; do
  echo "== newcomer in the app's $lv Europe (information)"
  for sd in 1 2 3; do
    echo "seed $sd: $(target/release/rr-cli soak --mission europe --seed $sd --newcomer 1 --level $lv --gf 108000 2>&1 | grep -E "newcomer at 90|player 0 defeated" | head -1)"
  done
done
echo "== ladder"
for p in "hard normal" "normal easy"; do set -- $p; target/release/rr-cli ladder --a "$1" --b "$2" --seeds 16 --gf 60000 | tail -1; done
# long games too: hard collapsed after an hour and the short ladder didn't show it (veteran round 8, #3)
echo "== ladder, 120 minutes (information)"
target/release/rr-cli ladder --a hard --b normal --seeds 16 --gf 144000 | tail -1
# each mission's goal as the app sets it up: a hard stand-in against easy rivals, seeds 1-3
echo "== mission goals, minute met (hard stand-in, easy rivals; information)"
cargo test -q --release -p rr-app --bin rr-app -- --ignored goals --nocapture 2>&1 | grep "^goal:" | sort | tr '\n' ' '; echo
exit $fail
