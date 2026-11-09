#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [[ $# != 2 || ! -d "$1/Contents" || "$(basename "$1")" != Superfik.app ]]; then
  echo "usage: $0 /path/to/Superfik.app /path/to/Superfik.dmg" >&2
  exit 1
fi
APP="$(cd "$(dirname "$1")" && pwd)/Superfik.app"
mkdir -p "$(dirname "$2")"
DMG="$(cd "$(dirname "$2")" && pwd)/$(basename "$2")"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/superfik-dmg.XXXXXX")"
MOUNT="$WORK/volume"
ATTACHED=0
cleanup() {
  if [[ "$ATTACHED" == 1 ]]; then
    if ! hdiutil detach -quiet "$MOUNT" && ! hdiutil detach -quiet -force "$MOUNT"; then
      echo "warning: could not eject $MOUNT; leaving temporary files at $WORK" >&2
      return
    fi
  fi
  rm -rf "$WORK"
}
trap cleanup EXIT

mkdir -p "$WORK/stage/.background"
ditto "$APP" "$WORK/stage/Superfik.app"
swift "$ROOT/assets/macos/dmg-background.swift" "$WORK/stage/.background/background.png"
hdiutil create -quiet -volname Superfik -fs HFS+ -srcfolder "$WORK/stage" \
  -format UDRW "$WORK/installer.dmg"
hdiutil attach -quiet -readwrite -noverify -noautoopen -nobrowse \
  -mountpoint "$MOUNT" "$WORK/installer.dmg"
ATTACHED=1
ln -s /Applications "$MOUNT/Applications"
osascript "$ROOT/assets/macos/dmg-layout.applescript" "$MOUNT"
if [[ ! -s "$MOUNT/.DS_Store" ]]; then
  echo "error: Finder did not save the installer layout; run in a logged-in macOS GUI session" >&2
  exit 1
fi
hdiutil detach -quiet "$MOUNT"
ATTACHED=0
hdiutil convert -quiet "$WORK/installer.dmg" -format UDZO -o "$WORK/final.dmg"
hdiutil verify -quiet "$WORK/final.dmg"
mv -f "$WORK/final.dmg" "$DMG"
echo "Built $DMG"
