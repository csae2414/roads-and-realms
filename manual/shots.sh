#!/usr/bin/env bash
# The manual's screenshots, headless (tools/shot.sh: a virtual X server, no sound), in the
# manual's language: manual/shots.sh de|en  ->  manual/build/<lang>/img/*.png
set -euo pipefail
cd "$(dirname "$0")/.."
lang="${1:-de}"
case "$lang" in de) export LANG=de_DE.UTF-8 ;; *) export LANG=en_US.UTF-8 ;; esac
img="manual/build/$lang/img"; mkdir -p "$img"
cargo build -q --release -p rr-app
shot() { # name RR_OPEN args...
  local name="$1" open="$2"; shift 2
  RR_OPEN="$open" tools/shot.sh "$img/$name.png" "$@" >/dev/null 2>&1 || echo "shot $name failed" >&2
}
E=(--mission europe)
shot menu        ""                       --menu
shot overview    play                     "${E[@]}" --gf 30000
shot start       play                     "${E[@]}" --gf 400 &
shot build       play,build               "${E[@]}" --gf 2000
shot build_mid   play,build:1             "${E[@]}" --gf 2000 &
shot bq          play,bq                  "${E[@]}" --gf 6000
shot prod        play,prod,names          "${E[@]}" --gf 30000 &
shot hq          play,hq                  "${E[@]}" --gf 6000
shot sawmill     play,kind:Sawmill        "${E[@]}" --gf 20000 &
shot farm        play,kind:Farm           "${E[@]}" --gf 30000
shot guardhouse  play,kind:Guardhouse     "${E[@]}" --gf 20000 &
shot mine        play,kind:CoalMine       "${E[@]}" --gf 40000
wait
shot main        play,main                "${E[@]}" --gf 20000 &
shot inventory   play,inventory           "${E[@]}" --gf 20000
shot distribution play,distribution       "${E[@]}" --gf 20000 &
shot transport   play,transport           "${E[@]}" --gf 20000
shot tools       play,tools               "${E[@]}" --gf 20000 &
shot military    play,military            "${E[@]}" --gf 20000
shot stats       play,stats:0             "${E[@]}" --gf 60000 &
shot stats_wares play,stats:10            "${E[@]}" --gf 60000
shot buildings   play,buildings           "${E[@]}" --gf 30000 &
shot map         play,map                 "${E[@]}" --gf 30000
shot options     play,options             "${E[@]}" --gf 2000 &
shot saves       play,saves               "${E[@]}" --gf 2000
shot help        play,help                "${E[@]}" --gf 2000
wait
shot first       ""                       --mission first &
shot north       play                     --mission north --gf 30000
shot fareast     play                     --mission fareast --gf 30000 &
shot america     play                     --mission northamerica --gf 30000
shot sea         play,kind:Harbor         --mission mediterranean --gf 90000 &
shot shipyard    play,kind:Shipyard       --mission mediterranean --gf 90000
shot wasteland   ""                       --landscape wasteland --gf 30000
wait
# the same frames without a window, for manual/crop.py to find each window by the difference
for gf in 2000 6000 20000 30000 40000 60000; do shot "base_$gf" play "${E[@]}" --gf $gf & done
shot base_med_90000 play --mission mediterranean --gf 90000
wait
# through the title menu, as a player gets there: the Europe mission's briefing, the map editor
ui() { # name script
  local dir; dir="$(mktemp -d)"
  SCRIPT="$2" tools/uitest.sh "$dir" --menu >/dev/null 2>&1 || true
  cp "$(ls "$dir"/shot_*.png | head -1)" "$img/$1.png" 2>/dev/null || echo "shot $1 failed" >&2
  rm -rf "$dir"
}
ui briefing "sleep 1; mousemove 478 222; click 1; sleep 2; shot"
ui editor   "sleep 1; mousemove 820 346; click 1; sleep 3; shot"
echo "shots in $img: $(ls "$img" | wc -l)"
