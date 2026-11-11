#!/usr/bin/env bash
set -euo pipefail

BINARY="$1"
if [[ -z "${MACOS_CERTIFICATE:-}" || -z "${APPLE_API_KEY:-}" ]]; then
  echo "no signing secrets: $BINARY stays unsigned"
  exit 0
fi
IDENTITY="Developer ID Application"
WORK="$(mktemp -d "${RUNNER_TEMP:-/tmp}/sign.XXXXXX")"
KEYCHAIN="$WORK/signing.keychain-db"
PASSWORD="$(uuidgen)"

cleanup() {
  security delete-keychain "$KEYCHAIN" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

security create-keychain -p "$PASSWORD" "$KEYCHAIN"
security set-keychain-settings -lut 21600 "$KEYCHAIN"
security unlock-keychain -p "$PASSWORD" "$KEYCHAIN"
echo "$MACOS_CERTIFICATE" | base64 --decode > "$WORK/certificate.p12"
security import "$WORK/certificate.p12" -k "$KEYCHAIN" -P "$MACOS_CERTIFICATE_PASSWORD" \
  -T /usr/bin/codesign -T /usr/bin/security
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$PASSWORD" "$KEYCHAIN" >/dev/null
security list-keychains -d user -s "$KEYCHAIN" $(security list-keychains -d user | tr -d '"')

codesign --force --options runtime --timestamp --keychain "$KEYCHAIN" --sign "$IDENTITY" "$BINARY"
codesign --verify --strict --verbose=2 "$BINARY"

echo "$APPLE_API_KEY" | base64 --decode > "$WORK/AuthKey.p8"
ditto -c -k "$BINARY" "$WORK/notarize.zip"
RESULT="$(xcrun notarytool submit "$WORK/notarize.zip" \
  --key "$WORK/AuthKey.p8" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER" \
  --wait --output-format json)"
echo "$RESULT"
STATUS="$(echo "$RESULT" | python3 -I -c 'import json, sys; print(json.load(sys.stdin)["status"])')"
if [[ "$STATUS" != "Accepted" ]]; then
  ID="$(echo "$RESULT" | python3 -I -c 'import json, sys; print(json.load(sys.stdin)["id"])')"
  xcrun notarytool log "$ID" \
    --key "$WORK/AuthKey.p8" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER" >&2 || true
  echo "error: notarization $STATUS" >&2
  exit 1
fi
