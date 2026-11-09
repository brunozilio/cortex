#!/usr/bin/env bash
set -euo pipefail

TEAM_ID=F3BSQ3FZTF
PROFILE=assets/macos/Superfik.provisionprofile
NOTARY=superfik
RELEASE="${SUPERFIK_RELEASE:-0}"
ADHOC="${SUPERFIK_ADHOC:-0}"
IDENTITY="Developer ID Application: Bruno Zilio ($TEAM_ID)"
if [[ "$ADHOC" == 1 && "$RELEASE" == 1 ]]; then
  echo "error: SUPERFIK_ADHOC cannot be used for a release" >&2
  exit 1
fi
KEYCHAIN_ARGS=()
if [[ -n "${SIGN_KEYCHAIN:-}" ]]; then
  KEYCHAIN_ARGS=(--keychain "$SIGN_KEYCHAIN")
fi
if [[ "$ADHOC" == 1 ]]; then
  IDENTITY=-
elif ! security find-identity -v -p codesigning ${SIGN_KEYCHAIN:+"$SIGN_KEYCHAIN"} | grep -qF "\"$IDENTITY\""; then
  if [[ "$RELEASE" == 1 ]]; then
    echo "error: no '$IDENTITY' in the keychain" >&2
    exit 1
  fi
  IDENTITY=-
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
if [[ -n "${SUPERFIK_BROWSER_RELEASE_LOCK:-}" ]]; then
  if [[ -n "${CEF_PATH:-}" ]]; then
    echo "error: select either CEF_PATH or SUPERFIK_BROWSER_RELEASE_LOCK" >&2
    exit 1
  fi
  CEF_PATH="$(python3 scripts/fetch_engine.py --lock "$SUPERFIK_BROWSER_RELEASE_LOCK")"
  export CEF_PATH
fi
if [[ -n "${CEF_PATH:-}" ]]; then
  CEF_PATH="$(python3 scripts/cef_artifact.py "$CEF_PATH" --lock "${SUPERFIK_BROWSER_RELEASE_LOCK:-scripts/cef-release.lock.json}" --probe)"
  export CEF_PATH
fi
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
OUT="$ROOT/target/release/bundle"
APP="$OUT/Superfik.app"

cargo build --release -p superfik-app

rm -rf "$APP" && mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/superfik "$APP/Contents/MacOS/superfik"
sed "s/__VERSION__/$VERSION/g" assets/macos/Info.plist > "$APP/Contents/Info.plist"
cargo run --quiet -p xtask -- cef-bundle "$APP" superfik

ICONSET="$OUT/Superfik.iconset"
rm -rf "$ICONSET"
cargo run --quiet -p xtask -- iconset "$ICONSET"
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/Superfik.icns"
rm -rf "$ICONSET"

cp LICENSE NOTICE assets/fonts/OFL.txt assets/fonts/FiraMono-OFL.txt \
  crates/superfik-ui/src/icons/LICENSE-hugeicons.md "$APP/Contents/Resources/"
cargo run -q -p xtask -- notices "$APP/Contents/Resources/THIRD_PARTY_NOTICES.md"

if [[ "$IDENTITY" == "-" ]]; then
  echo "Ad-hoc signing (local/CI use only)" >&2
  SIGN=(codesign --force --sign -)
else
  SIGN=(codesign --force --options runtime --timestamp ${KEYCHAIN_ARGS[@]+"${KEYCHAIN_ARGS[@]}"} --sign "$IDENTITY")
fi
CEF="$APP/Contents/Frameworks/Chromium Embedded Framework.framework"
while IFS= read -r -d '' lib; do
  if file -b "$lib" | grep -q Mach-O; then
    "${SIGN[@]}" "$lib"
  fi
done < <(find "$CEF/Libraries" -type f -print0 2>/dev/null)
"${SIGN[@]}" "$CEF"
for helper in "$APP"/Contents/Frameworks/*Helper*.app; do
  "${SIGN[@]}" --entitlements assets/macos/helper-entitlements.plist "$helper"
done
ENTITLEMENTS="$OUT/entitlements.plist"
if [[ "$IDENTITY" != "-" && -f "$PROFILE" ]]; then
  sed "s/__TEAM_ID__/$TEAM_ID/g" assets/macos/entitlements.plist > "$ENTITLEMENTS"
  cp "$PROFILE" "$APP/Contents/embedded.provisionprofile"
else
  [[ "$IDENTITY" != "-" ]] && echo "warning: no provisioning profile at $PROFILE; Touch ID passkeys disabled" >&2
  sed '/Chromium.s passkey store/,/<\/array>/d' assets/macos/entitlements.plist > "$ENTITLEMENTS"
fi
"${SIGN[@]}" --entitlements "$ENTITLEMENTS" "$APP"
codesign --verify --strict "$APP"

DMG="$OUT/Superfik-$VERSION.dmg"
bash scripts/create-dmg.sh "$APP" "$DMG"
if [[ "$IDENTITY" != "-" ]]; then
  codesign ${KEYCHAIN_ARGS[@]+"${KEYCHAIN_ARGS[@]}"} --sign "$IDENTITY" --timestamp "$DMG"
fi
if [[ "$IDENTITY" == "-" ]]; then
  :
elif [[ -n "${NOTARY_KEY:-}" ]]; then
  API=(--key "$NOTARY_KEY" --key-id "$NOTARY_KEY_ID" --issuer "$NOTARY_ISSUER")
  SUBMITTED="$(xcrun notarytool submit "$DMG" "${API[@]}" --wait --output-format json)"
  echo "$SUBMITTED"
  if ! grep -q '"status" *: *"Accepted"' <<<"$SUBMITTED"; then
    ID="$(sed -n 's/.*"id" *: *"\([^"]*\)".*/\1/p' <<<"$SUBMITTED")"
    xcrun notarytool log "$ID" "${API[@]}" >&2 || true
    echo "error: notarization was not accepted" >&2
    exit 1
  fi
  xcrun stapler staple "$DMG"
elif xcrun notarytool history --keychain-profile "$NOTARY" >/dev/null 2>&1; then
  xcrun notarytool submit "$DMG" --keychain-profile "$NOTARY" --wait
  xcrun stapler staple "$DMG"
elif [[ "$RELEASE" == 1 ]]; then
  echo "error: no notarytool profile '$NOTARY'" >&2
  exit 1
else
  echo "warning: no notarytool profile '$NOTARY'; not notarized" >&2
fi

if [[ "$IDENTITY" != "-" ]] && xcrun stapler validate "$DMG" >/dev/null 2>&1; then
  xcrun stapler staple "$APP" || echo "warning: could not staple the app; the update is notarized online only" >&2
fi
case "$(uname -m)" in
  arm64 | aarch64) UPDATE_ARCH=aarch64 ;;
  x86_64) UPDATE_ARCH=x86_64 ;;
  *) echo "error: unsupported architecture $(uname -m)" >&2; exit 1 ;;
esac
ZIP="$OUT/Superfik-$VERSION-macos-$UPDATE_ARCH.zip"
rm -f "$ZIP"
ditto -c -k --keepParent "$APP" "$ZIP"
echo "Built $APP"
echo "Built $DMG"
echo "Built $ZIP"
