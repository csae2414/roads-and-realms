#!/usr/bin/env bash
# Local CI: format, lint, test. Usage: tools/check.sh
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
# exempt long functions may not grow (architect round 3, #10)
python3 tools/fnlen.py
cargo clippy --workspace --all-targets -q -- -D warnings
cargo test --workspace -q
