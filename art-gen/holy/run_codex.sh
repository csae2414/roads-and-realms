#!/usr/bin/env bash
# every holy place of list.txt without a picture yet, three at a time
cd "$(dirname "$0")"
grep -v '^$' list.txt | while IFS='|' read -r key desc; do
  [ -f "$key.png" ] && continue
  printf '%s\0%s\0' "$key" "$desc"
done | xargs -0 -n2 -P3 ./gen_codex.sh
