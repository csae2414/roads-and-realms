#!/usr/bin/env bash
# Drive the game in a virtual X server with xdotool and save screenshots (F12).
# usage: SCRIPT="sleep 1; shot; mousemove 640 362 click 1; shot" tools/uitest.sh OUTDIR [app args]
# RR_SCREEN=1366x768 RR_WINDOW=1366x700 test another screen size
set -euo pipefail
out="$1"; shift
mkdir -p "$out"
export RR_SHOT_DIR="$(realpath "$out")"
cd "$(dirname "$0")/.."
# saves and settings go to a fresh folder, never the player's own (XDG_DATA_HOME_TEST picks one)
export XDG_DATA_HOME="${XDG_DATA_HOME_TEST:-$(mktemp -d)}"
# no sound on the machine's speakers: the default audio device is a null one (a test run
# played the game's sound on the owner's machine, perf round 2)
export ALSA_CONFIG_PATH="$(realpath tools/asound-null.conf)"
timeout 300 env -u WAYLAND_DISPLAY xvfb-run -a -s "-screen 0 ${RR_SCREEN:-1280x800}x24" bash -c '
  target/release/rr-app "$@" 2>>"$RR_SHOT_DIR/log.txt" & pid=$!
  win=""
  for i in $(seq 1 40); do
    win=$(xdotool search --pid $pid 2>/dev/null | tail -1 || true)
    [ -n "$win" ] && break
    sleep 0.25
  done
  sleep 1.5
  xdotool windowfocus "$win" 2>/dev/null || true
  xdotool getwindowgeometry "$win" >> "$RR_SHOT_DIR/log.txt" 2>&1 || true
  xdotool search --name "" getwindowname %@ >> "$RR_SHOT_DIR/log.txt" 2>&1 || true
  IFS=";" read -ra steps <<< "$SCRIPT"
  for s in "${steps[@]}"; do
    s="$(echo "$s" | xargs)"
    case "$s" in
      sleep*) sleep ${s#sleep } ;;
      shot) touch "$RR_SHOT_DIR/trigger"; sleep 1.0 ;;
      key*) xdotool key ${s#key } ;;
      *) xdotool $s ;;
    esac
  done
  kill $pid 2>/dev/null || true
' _ "$@"
