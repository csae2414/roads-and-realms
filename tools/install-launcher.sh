#!/usr/bin/env bash
# Put Roads & Realms in the desktop's app grid and dock (Ubuntu / GNOME, any freedesktop
# desktop), for this user only: the icon in every size and a .desktop launcher.
#
#   tools/install-launcher.sh             from the source tree (starts target/release/rr-app)
#   ./install-launcher.sh                 from an unpacked package (starts the rr-app beside it)
#   tools/install-launcher.sh --remove    takes the launcher and icons away again
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
id=roads-and-realms
data="${XDG_DATA_HOME:-$HOME/.local/share}"
apps="$data/applications"
icons="$data/icons/hicolor"

if [ "${1:-}" = "--remove" ]; then
  rm -f "$apps/$id.desktop"
  for n in 16 24 32 48 64 128 256 512; do rm -f "$icons/${n}x${n}/apps/$id.png"; done
  echo "Launcher removed."
  exit 0
fi

# the game: beside this script (a package), or the source tree's release build
if [ -x "$here/rr-app" ]; then
  root="$here"; exe="$here/rr-app"
else
  root="$(cd "$here/.." && pwd)"; exe="$root/target/release/rr-app"
  [ -x "$exe" ] || (cd "$root" && cargo build --release -p rr-app)
fi
src="$root/assets/icon"
[ -f "$src/$id-256.png" ] || { echo "No icon in $src" >&2; exit 1; }

for n in 16 24 32 48 64 128 256 512; do
  mkdir -p "$icons/${n}x${n}/apps"
  cp "$src/$id-$n.png" "$icons/${n}x${n}/apps/$id.png"
done
mkdir -p "$apps"
cat > "$apps/$id.desktop" <<EOF
[Desktop Entry]
Type=Application
Version=1.5
Name=Roads & Realms
GenericName=Settlement strategy game
GenericName[de]=Aufbaustrategiespiel
Comment=Build roads, grow a realm, and take the castles of your rivals
Comment[de]=Baue Straßen, lass ein Reich wachsen und nimm die Burgen deiner Gegner ein
Exec="$exe"
Path=$root
Icon=$id
Terminal=false
Categories=Game;StrategyGame;
Keywords=settlers;strategy;economy;medieval;
StartupWMClass=$id
StartupNotify=true
EOF
chmod +x "$apps/$id.desktop"
# (both are optional: GNOME notices new launchers by itself)
command -v update-desktop-database >/dev/null && update-desktop-database "$apps" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$icons" 2>/dev/null || true
echo "Installed: Roads & Realms in the app grid (Super, then type \"Roads\"); it starts $exe"
