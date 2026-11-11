#!/bin/sh
f=$(jq -r '.tool_input.file_path // empty')
case "$f" in
  *.rs) rustfmt --config-path "$CLAUDE_PROJECT_DIR/rustfmt.toml" "$f" >/dev/null 2>&1 ;;
esac
exit 0
