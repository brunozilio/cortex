#!/bin/sh
command=$(jq -r '.tool_input.command // empty')
case "$command" in
  *"gh pr create"*) ;;
  *) exit 0 ;;
esac
if ! "$(dirname "$0")/pr-gate.sh" >/dev/null; then
  echo "The PR gate failed (above): fix it, then open the PR." >&2
  exit 2
fi
exit 0
