#!/bin/sh
set -eu

UPDATE_KEY_PEM='-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEARHq/KW8PeuJNgkCCagKeAk3iqPMtnNVVgeM6Q1SY+lY=
-----END PUBLIC KEY-----'
LIB=/usr/local/lib/superfik-server

die() {
  echo "install-server: $*" >&2
  exit 1
}

value() {
  [ "$2" -ge 2 ] || die "$1 needs a value"
  case "$3" in '' | -*) die "$1 needs a value" ;; esac
}

DOMAIN=""
VERSION=""
REPOSITORY="brunozilio/superfik"
KEY_FILE=""
VERIFY=""
while [ $# -gt 0 ]; do
  case "$1" in
    --domain) value "$1" "$#" "${2-}"; DOMAIN="$2"; shift 2 ;;
    --version) value "$1" "$#" "${2-}"; VERSION="${2#v}"; shift 2 ;;
    --repository) value "$1" "$#" "${2-}"; REPOSITORY="$2"; shift 2 ;;
    --public-key) value "$1" "$#" "${2-}"; KEY_FILE="$2"; shift 2 ;;
    --verify) value "$1" "$#" "${2-}"; VERIFY="$2"; shift 2 ;;
    *) die "unknown option: $1" ;;
  esac
done

case "$DOMAIN" in *[!A-Za-z0-9.-]*) die "--domain takes a host name, like superfik.example.com" ;; esac
case "$VERSION" in *[!0-9A-Za-z.+-]*) die "--version takes a version, like 1.2.3" ;; esac
case "$REPOSITORY" in
  */*/* | /* | */ | *[!A-Za-z0-9._/-]*) die "--repository takes OWNER/NAME" ;;
  */*) ;;
  *) die "--repository takes OWNER/NAME" ;;
esac

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
if [ -n "$KEY_FILE" ]; then
  [ -f "$KEY_FILE" ] || die "no public key at $KEY_FILE"
  cp "$KEY_FILE" "$WORK/key.pem"
else
  printf '%s\n' "$UPDATE_KEY_PEM" >"$WORK/key.pem"
fi

verify() {
  command -v openssl >/dev/null 2>&1 || die "openssl is needed to check the download"
  case "$(openssl version)" in
    OpenSSL\ [3-9]* | OpenSSL\ [1-9][0-9]*) ;;
    *) die "OpenSSL 3 or newer is needed to check the download (found: $(openssl version))" ;;
  esac
  [ -f "$1.sig" ] || die "no signature beside $1"
  openssl dgst -sha256 -binary -out "$WORK/digest" "$1"
  if ! openssl pkeyutl -verify -pubin -inkey "$WORK/key.pem" -rawin \
    -in "$WORK/digest" -sigfile "$1.sig" >/dev/null 2>&1; then
    die "$1 is not signed with the release key: not installing it"
  fi
}

if [ -n "$VERIFY" ]; then
  verify "$VERIFY"
  echo "$VERIFY: signature verified"
  exit 0
fi

[ "$(id -u)" = 0 ] || die "run it as root (sudo)"
if [ "$(uname -s)" != Linux ] || [ "$(uname -m)" != x86_64 ]; then
  die "Linux x86-64 only for now; see docs/self-hosting.md for Docker"
fi

echo "Installing what the server needs…"
if command -v apt-get >/dev/null 2>&1; then
  apt-get update -qq
  apt-get install -y -qq ca-certificates curl openssl git sudo libglib2.0-0t64 libnspr4 libnss3 libatk1.0-0t64 \
    libatk-bridge2.0-0t64 libatspi2.0-0t64 libdbus-1-3 libcups2t64 libx11-6 libxcomposite1 libxdamage1 \
    libxext6 libxfixes3 libxrandr2 libgbm1 libexpat1 libxcb1 libxkbcommon0 libcairo2 libpango-1.0-0 \
    libudev1 libasound2t64 libvulkan1 mesa-vulkan-drivers fonts-noto-core >/dev/null
else
  echo "No apt-get: install curl, openssl (3+), git, sudo and CEF's libraries yourself (see deploy/Dockerfile)." >&2
fi

fetch() {
  curl --proto '=https' --tlsv1.2 -fsSL "$@"
}

if [ -z "$VERSION" ]; then
  VERSION="$(fetch "https://api.github.com/repos/$REPOSITORY/releases/latest" |
    sed -n 's/.*"tag_name": *"v\{0,1\}\([0-9A-Za-z.+-]*\)".*/\1/p' | head -n 1)"
fi
[ -n "$VERSION" ] || die "could not find the latest release of $REPOSITORY"
NAME="superfik-server-$VERSION-linux-x86_64"
URL="https://github.com/$REPOSITORY/releases/download/v$VERSION/$NAME.tar.gz"

echo "Downloading Superfik server $VERSION…"
fetch "$URL" -o "$WORK/$NAME.tar.gz"
fetch "$URL.sig" -o "$WORK/$NAME.tar.gz.sig"
verify "$WORK/$NAME.tar.gz"
echo "Signature verified."

mkdir "$WORK/out"
tar --no-same-owner -C "$WORK/out" -xzf "$WORK/$NAME.tar.gz"
[ -x "$WORK/out/$NAME/superfik-server" ] || die "the archive has no $NAME/superfik-server"
chown -R root:root "$WORK/out/$NAME"
chmod -R go-w "$WORK/out/$NAME"
chmod 0755 "$WORK/out/$NAME"
mkdir -p "$(dirname "$LIB")" /usr/local/bin
rm -rf "$LIB.new"
mv "$WORK/out/$NAME" "$LIB.new"
rm -rf "$LIB"
mv "$LIB.new" "$LIB"
for bin in superfik-server superfik-native-worker superfik-session-migrate; do
  ln -sfn "$LIB/$bin" "/usr/local/bin/$bin"
done

set -- install
if [ -n "$DOMAIN" ]; then
  set -- "$@" --domain "$DOMAIN"
fi
exec "$LIB/superfik-server" "$@"
