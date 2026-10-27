#!/usr/bin/env bash
cd "$(dirname "$0")"
while IFS='|' read -r key desc; do
  [ -f "$key.png" ] && continue
  ./gen.sh "$key" "$desc"
  echo "$(date +%T) $key $( [ -f $key.png ] && echo ok || echo FAIL)" >> progress.txt
done < list.txt
