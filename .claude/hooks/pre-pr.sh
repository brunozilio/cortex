#!/bin/sh
# PreToolUse hook (Bash), for Claude Code and Superfik alike: before
# `gh pr create` runs, the PR gate must pass. A failure blocks the command
# (exit 2) and the gate's report goes back to the agent. Every other
# command passes straight through.
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
