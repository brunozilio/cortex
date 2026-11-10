#!/usr/bin/env bash
set -euo pipefail
: "${CEF_PATH:?Set CEF_PATH to the validated Superfik engine artifact}"
if [[ -n "${SUPERFIK_BROWSER_CHECK_LOGS:-}" ]]; then
  logs_path="$SUPERFIK_BROWSER_CHECK_LOGS"
  mkdir -p "$logs_path"
else
  logs_path="$(mktemp -d "${TMPDIR:-/tmp}/superfik-browser-check.XXXXXX")"
  trap 'rm -rf "$logs_path"' EXIT
fi
run_case() {
  local case_name="$1"
  shift
  if env "$@" cargo run -q -p superfik-browser --example native_smoke > "$logs_path/$case_name.log" 2>&1; then
    echo "Native browser: $case_name passed"
  else
    tail -100 "$logs_path/$case_name.log"
    echo "Native browser: $case_name failed" >&2
    return 1
  fi
}
run_case failure SUPERFIK_HOST_FAILURE_SMOKE=1
run_case agent SUPERFIK_AGENT_SMOKE=1
for mode in INTERACTION CLIPBOARD WAIT OAUTH FILES DIALOG NAVIGATION COOKIE CLOSE FILL CAPTURE PASSKEY; do
  run_case "$mode" SUPERFIK_AGENT_SMOKE=1 "SUPERFIK_${mode}_SMOKE=1"
done
run_case profiles SUPERFIK_EXTENDED_PROFILE_SMOKE=1 "SUPERFIK_BROWSER_SOFTWARE_GL=${SUPERFIK_PROFILES_ON_SOFTWARE_GL:-0}"
run_case proxy SUPERFIK_PROXY_SMOKE=1
