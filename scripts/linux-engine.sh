#!/usr/bin/env bash
prepare_linux_engine() {
  local rust_arch="$1"
  local engine_target="$rust_arch-unknown-linux-gnu"
  local engine_lock="${SUPERFIK_BROWSER_RELEASE_LOCK:-scripts/cef-release.linux-$rust_arch.lock.json}"
  if [[ "$(uname -s)" != Linux ]]; then
    echo "error: build Linux packages on Linux" >&2
    return 1
  fi
  if [[ -n "${SUPERFIK_BROWSER_RELEASE_LOCK:-}" && -n "${CEF_PATH:-}" ]]; then
    echo "error: select either CEF_PATH or SUPERFIK_BROWSER_RELEASE_LOCK" >&2
    return 1
  fi
  if [[ -z "${CEF_PATH:-}" ]]; then
    CEF_PATH="$(python3 scripts/fetch_engine.py --lock "$engine_lock" --target "$engine_target")" || return
    export CEF_PATH
  fi
  if [[ "${SUPERFIK_RELEASE:-0}" == 1 || -n "${SUPERFIK_BROWSER_RELEASE_LOCK:-}" ]]; then
    CEF_PATH="$(python3 scripts/cef_artifact.py "$CEF_PATH" --lock "$engine_lock" --target "$engine_target" --probe)" || return
    export CEF_PATH
  fi
  if [[ ! -f "$CEF_PATH/CREDITS.html" ]]; then
    echo "error: CEF_PATH must name the full distribution, including CREDITS.html" >&2
    return 1
  fi
}
