#!/bin/sh
set -u
pid="$1"
current="$2"
staged="$3"
shift 3

waited=0
while kill -0 "$pid" 2>/dev/null; do
  waited=$((waited + 1))
  if [ "$waited" -gt 600 ]; then
    exit 1
  fi
  sleep 0.1
done

backup="$current.superfik-old"
rm -rf "$backup"
if mv "$current" "$backup"; then
  if mv "$staged" "$current"; then
    rm -rf "$backup"
  else
    mv "$backup" "$current"
  fi
fi
rm -rf "$(dirname "$staged")"
rm -f "$0"
rmdir "$(dirname "$0")" 2>/dev/null
"$@" >/dev/null 2>&1 &
exit 0
