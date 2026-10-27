#!/usr/bin/env bash
# usage: tools/dep.sh <crate> '<dependency line>'   — inserts the line under [dependencies]
set -euo pipefail
f="$(dirname "$0")/../crates/$1/Cargo.toml"
grep -qF -- "$2" "$f" || sed -i "/^\[dependencies\]$/a $2" "$f"
