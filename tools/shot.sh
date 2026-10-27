#!/usr/bin/env bash
# Headless screenshot in a virtual X server (never on the real desktop).
# usage: tools/shot.sh OUT.png [app args...]
set -euo pipefail
out="$1"; shift
cd "$(dirname "$0")/.."
# saves and settings go to a fresh folder, never the player's own (XDG_DATA_HOME_TEST picks one)
export XDG_DATA_HOME="${XDG_DATA_HOME_TEST:-$(mktemp -d)}"
# no sound on the machine's speakers: the default audio device is a null one (a test run
# played the game's sound on the owner's machine, perf round 2)
export ALSA_CONFIG_PATH="$(realpath tools/asound-null.conf)"
timeout 300 env -u WAYLAND_DISPLAY xvfb-run -a -s "-screen 0 ${RR_SCREEN:-1280x800}x24" target/release/rr-app --screenshot "$out" "$@"
