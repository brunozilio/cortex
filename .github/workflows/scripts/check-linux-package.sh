#!/usr/bin/env bash
set -euo pipefail

DEB="$(realpath "${1:?Pass the .deb package}")"
ARCHIVE="$(realpath "${2:?Pass the portable .tar.gz}")"
EVIDENCE="$(realpath -m "${3:?Pass an evidence directory}")"
mkdir -p "$EVIDENCE"
TEMP="$(mktemp -d)"
trap 'rm -rf "$TEMP"' EXIT

[[ "$(dpkg-deb -f "$DEB" Package)" == superfik ]]
VERSION="$(dpkg-deb -f "$DEB" Version)"
ARCH="$(dpkg-deb -f "$DEB" Architecture)"
case "$(uname -m):$ARCH" in
  x86_64:amd64 | aarch64:arm64) ;;
  *) echo "Package architecture does not match the test machine" >&2; exit 1 ;;
esac
dpkg-deb --info "$DEB" > "$EVIDENCE/deb-info.txt"
dpkg-deb --contents "$DEB" > "$EVIDENCE/deb-contents.txt"
dpkg-deb --extract "$DEB" "$TEMP/deb"
mkdir "$TEMP/tar"
tar -xzf "$ARCHIVE" -C "$TEMP/tar"
PORTABLE="$TEMP/tar/superfik-$VERSION"
cmp "$TEMP/deb/usr/lib/superfik/superfik" "$PORTABLE/usr/lib/superfik/superfik"

for format in deb tar; do
  ROOT="$TEMP/deb"
  if [[ "$format" == tar ]]; then ROOT="$PORTABLE"; fi
  BIN="$ROOT/usr/bin/superfik"
  [[ -x "$BIN" && -L "$BIN" ]]
  [[ "$(realpath "$BIN")" == "$ROOT/usr/lib/superfik/superfik" ]]
  for notice in copyright THIRD_PARTY_NOTICES.md OFL.txt Chromium-CREDITS.html; do
    [[ -s "$ROOT/usr/share/doc/superfik/$notice" ]] || {
      echo "$format: missing attribution $notice" >&2; exit 1;
    }
  done
  for file in libcef.so icudtl.dat v8_context_snapshot.bin resources.pak locales/en-US.pak; do
    [[ -s "$ROOT/usr/lib/superfik/$file" ]]
  done
  ldd "$(realpath "$BIN")" > "$EVIDENCE/$format-ldd.txt" 2>&1
  if grep -q 'not found' "$EVIDENCE/$format-ldd.txt"; then
    cat "$EVIDENCE/$format-ldd.txt" >&2
    exit 1
  fi
  for theme in dark light; do
    IMAGE="$EVIDENCE/$format-$theme.png"
    timeout 120 "$BIN" --snapshot "$IMAGE" --scenario reference \
      --size 1440x900 --scale 1 --theme "$theme" --lang pt-BR \
      > "$EVIDENCE/$format-$theme.log" 2>&1 || {
        cat "$EVIDENCE/$format-$theme.log" >&2; exit 1;
      }
    python3 - "$IMAGE" <<'PY'
from PIL import Image
import sys
image = Image.open(sys.argv[1]).convert("RGB")
assert image.size == (1440, 900), image.size
assert len(image.getcolors(image.width * image.height)) > 100, "blank/flat frame"
PY
  done
  echo "$format: runtime dependencies, browser files, licences and rendering passed"
done
echo "Linux package checks passed: $VERSION ($ARCH); evidence: $EVIDENCE"
