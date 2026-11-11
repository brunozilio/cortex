#!/usr/bin/env bash
set -euo pipefail
root="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel)}"
file="$(python3 -c 'import json,sys; print(json.load(sys.stdin).get("tool_input",{}).get("file_path",""))' 2>/dev/null || true)"
case "$file" in
  *.rs|*.sh|*.py|*.toml) ;;
  *) exit 0 ;;
esac
[ -f "$file" ] || exit 0
if ! out="$(cd "$root" && CARGO_TARGET_DIR="$root/target/agents/hooks" cargo run -q -p xtask -- comments "$file" 2>&1)"; then
  printf '%s\n' "$out" >&2
  exit 2
fi
