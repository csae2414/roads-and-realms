#!/usr/bin/env bash
# Build a release tarball: binary + assets + licenses. Usage: tools/package.sh [version]
set -euo pipefail
cd "$(dirname "$0")/.."
v="${1:-$(date +%Y%m%d)}"
cargo build --release -p rr-app -p rr-cli
d="local/dist/roads-and-realms-$v"
rm -rf "$d"; mkdir -p "$d"
cp target/release/rr-app target/release/rr-cli "$d/"
cp -r assets "$d/assets"
cp LICENSE README.md "$d/"
# the desktop launcher (Ubuntu's app grid and dock): ./install-launcher.sh in the unpacked folder
cp tools/install-launcher.sh "$d/"
tar -C local/dist -czf "local/dist/roads-and-realms-$v-linux-x86_64.tar.gz" "roads-and-realms-$v"
ls -la "local/dist/roads-and-realms-$v-linux-x86_64.tar.gz"
