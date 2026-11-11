#!/bin/sh
set -u
root=${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel)}
cd "$root" || exit 1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target/agents/pr-gate}"
mkdir -p "$CARGO_TARGET_DIR"

stamp=$({ git rev-parse HEAD; git diff HEAD --no-ext-diff; git ls-files --others --exclude-standard; } | shasum | cut -d' ' -f1)
passed="$CARGO_TARGET_DIR/pr-gate.passed"
if [ "$(cat "$passed" 2>/dev/null)" = "$stamp" ]; then
  echo "ok    gate (already passed for this tree)"
  exit 0
fi

log=$(mktemp)
trap 'rm -f "$log"' EXIT
step() {
  name=$1
  shift
  if "$@" >"$log" 2>&1; then
    echo "ok    $name"
  else
    echo "FAIL  $name: $*" >&2
    tail -n 60 "$log" >&2
    exit 1
  fi
}

if command -v cargo-nextest >/dev/null 2>&1; then
  step tests cargo nextest run --workspace
  step doctests cargo test --doc --workspace
else
  step tests cargo test --workspace
fi
step clippy cargo clippy --workspace --all-targets -- -D warnings
step lint cargo run -q -p xtask -- lint

echo "$stamp" >"$passed"
