#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
case "$(uname -m)" in
  x86_64) ARCH=amd64; RUST_ARCH=x86_64 ;;
  aarch64 | arm64) ARCH=arm64; RUST_ARCH=aarch64 ;;
  *) echo "error: unsupported architecture $(uname -m)" >&2; exit 1 ;;
esac

source scripts/linux-engine.sh
prepare_linux_engine "$RUST_ARCH"

cargo build --release --locked -p superfik-app
TARGET="${CARGO_TARGET_DIR:-target}"
BINARY="$TARGET/release/superfik"
OUT="$TARGET/release/bundle"
STAGE="$OUT/linux-root"
rm -rf "$STAGE"
mkdir -p "$OUT"

LIB="$STAGE/usr/lib/superfik"
install -Dm755 "$BINARY" "$LIB/superfik"
for file in libcef.so libvk_swiftshader.so libvulkan.so.1; do
  install -Dm755 "$TARGET/release/$file" "$LIB/$file"
done
for file in vk_swiftshader_icd.json icudtl.dat v8_context_snapshot.bin resources.pak \
  chrome_100_percent.pak chrome_200_percent.pak; do
  install -Dm644 "$TARGET/release/$file" "$LIB/$file"
done
mkdir -p "$LIB/locales"
install -m644 "$TARGET/release/locales/"*.pak "$LIB/locales/"
strip --strip-unneeded "$LIB/superfik" "$LIB/libcef.so" "$LIB/libvk_swiftshader.so"
mkdir -p "$STAGE/usr/bin"
ln -s ../lib/superfik/superfik "$STAGE/usr/bin/superfik"
install -Dm644 assets/linux/superfik.desktop "$STAGE/usr/share/applications/superfik.desktop"
for size in 128 256 512; do
  install -Dm644 "assets/linux/superfik-$size.png" \
    "$STAGE/usr/share/icons/hicolor/${size}x${size}/apps/superfik.png"
done
DOC="$STAGE/usr/share/doc/superfik"
for file in LICENSE NOTICE assets/fonts/OFL.txt assets/fonts/FiraMono-OFL.txt \
  crates/superfik-ui/src/icons/LICENSE-hugeicons.md; do
  install -Dm644 "$file" "$DOC/$(basename "$file")"
done
install -m644 LICENSE "$DOC/copyright"
cargo run -q -p xtask -- notices "$DOC/THIRD_PARTY_NOTICES.md"
install -m644 "$CEF_PATH/CREDITS.html" "$DOC/Chromium-CREDITS.html"

TARBALL="$OUT/Superfik-$VERSION-linux-$ARCH.tar.gz"
tar -C "$STAGE" --transform "flags=rhS;s,^\.,superfik-$VERSION," -czf "$TARBALL" .

mkdir -p "$STAGE/DEBIAN"
SIZE="$(du -sk "$STAGE/usr" | cut -f1)"
cat > "$STAGE/DEBIAN/control" <<EOF
Package: superfik
Version: $VERSION
Architecture: $ARCH
Maintainer: Bruno Zilio <bruno@sofik.ai>
Installed-Size: $SIZE
Section: devel
Priority: optional
Homepage: https://superfik.com
Depends: libc6 (>= 2.39), libstdc++6 (>= 14), libgcc-s1, libasound2t64 | libasound2, libssl3t64 | libssl3, zlib1g, libzstd1, libxkbcommon0, libxkbcommon-x11-0, libx11-6, libx11-xcb1, libxcursor1, libxrandr2, libxi6, libwayland-client0, libwayland-cursor0, libvulkan1, xdg-utils,
 libglib2.0-0t64 | libglib2.0-0, libnspr4, libnss3, libatk1.0-0t64 | libatk1.0-0, libatk-bridge2.0-0t64 | libatk-bridge2.0-0, libatspi2.0-0t64 | libatspi2.0-0, libdbus-1-3, libcups2t64 | libcups2, libxcomposite1, libxdamage1, libxext6, libxfixes3, libgbm1, libexpat1, libxcb1, libcairo2, libpango-1.0-0, libudev1
Recommends: mesa-vulkan-drivers, xdg-desktop-portal, xdg-desktop-portal-gtk | xdg-desktop-portal-kde | xdg-desktop-portal-gnome, gnome-keyring | kwalletmanager, git
Description: A GPU canvas for AI coding agents
 Superfik shows what an agent sees and does around the composer:
 context, sub-agents, memory and reviews, on one canvas.
EOF
DEB="$OUT/superfik_${VERSION}_$ARCH.deb"
dpkg-deb --root-owner-group --build "$STAGE" "$DEB" >/dev/null
rm -rf "$STAGE"
(cd "$OUT" && sha256sum "$(basename "$DEB")" "$(basename "$TARBALL")" \
  > "Superfik-$VERSION-linux-$ARCH.sha256")
echo "$DEB"
echo "$TARBALL"
