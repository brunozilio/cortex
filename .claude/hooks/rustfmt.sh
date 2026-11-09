#!/bin/sh
# PostToolUse hook: format a just-edited .rs file with the workspace rustfmt.toml.
f=$(jq -r '.tool_input.file_path // empty')
case "$f" in
  # Vendored crates keep their upstream formatting (a diff stays reviewable).
  */vendor/*) ;;
  *.rs) rustfmt --config-path "$CLAUDE_PROJECT_DIR/rustfmt.toml" "$f" >/dev/null 2>&1 ;;
esac
exit 0
