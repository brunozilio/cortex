#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
case "$(uname -m)" in
  x86_64) RUST_ARCH=x86_64 ;;
  aarch64 | arm64) RUST_ARCH=aarch64 ;;
  *) echo "error: unsupported architecture $(uname -m)" >&2; exit 1 ;;
esac
source scripts/linux-engine.sh
prepare_linux_engine "$RUST_ARCH"

VERSION="$(cargo metadata --no-deps --format-version 1 | python3 -I -c 'import json, sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "superfik-server"))')"
cargo build --release --locked -p superfik-server -p superfik-native \
  --bin superfik-server --bin superfik-native-worker --bin superfik-session-migrate

NAME="superfik-server-$VERSION-linux-$RUST_ARCH"
BUILD="${CARGO_TARGET_DIR:-target}/release"
OUT="$BUILD/bundle/$NAME"
rm -rf "$OUT" && mkdir -p "$OUT/locales"
for file in superfik-server superfik-native-worker superfik-session-migrate \
  libcef.so libvk_swiftshader.so libvulkan.so.1 vk_swiftshader_icd.json icudtl.dat \
  v8_context_snapshot.bin resources.pak chrome_100_percent.pak chrome_200_percent.pak; do
  cp "$BUILD/$file" "$OUT/"
done
cp "$BUILD/locales/"*.pak "$OUT/locales/"
install -m 0755 deploy/native-as-runner "$OUT/native-as-runner"
mkdir -p "$OUT/licenses"
install -m 0644 LICENSE NOTICE "$CEF_PATH/CREDITS.html" "$OUT/licenses/"
cargo run -q -p xtask -- notices "$OUT/licenses/THIRD_PARTY_NOTICES.md"
strip --strip-unneeded "$OUT/superfik-server" "$OUT/libcef.so" "$OUT/libvk_swiftshader.so"
chmod -R go-w "$OUT"
tar -C "$BUILD/bundle" --owner=0 --group=0 --numeric-owner \
  -czf "$BUILD/bundle/$NAME.tar.gz" "$NAME"
(cd "$BUILD/bundle" && sha256sum "$NAME.tar.gz" > "$NAME.sha256")
echo "$BUILD/bundle/$NAME.tar.gz"
