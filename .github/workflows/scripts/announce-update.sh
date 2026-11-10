#!/usr/bin/env bash
set -euo pipefail

ARCHIVE="$1"
PLATFORM="$2"
if [[ -z "${SUPERFIK_UPDATE_KEY:-}" ]]; then
  echo "::warning::SUPERFIK_UPDATE_KEY is not set; $PLATFORM update not announced"
  exit 0
fi
VERSION="${TAG#v}"
FILE="$(basename "$ARCHIVE")"
REPOSITORY="${REPOSITORY:-${GITHUB_REPOSITORY:-brunozilio/superfik}}"
URL="https://github.com/$REPOSITORY/releases/download/$TAG/$FILE"

ASSET="$(cargo run --quiet -p xtask -- sign-update "$ARCHIVE" --version "$VERSION" --platform "$PLATFORM" --url "$URL")"
MANIFEST_DIR="$(mktemp -d)"
trap 'rm -rf "$MANIFEST_DIR"' EXIT
MANIFEST="$MANIFEST_DIR/superfik-$PLATFORM.json"
PYTHON=python3
"$PYTHON" -c '' 2>/dev/null || PYTHON=python
ASSET="$ASSET" VERSION="$VERSION" MINIMUM="${MINIMUM:-}" TAG="$TAG" REPOSITORY="$REPOSITORY" "$PYTHON" -I -c '
import json, os, time
print(json.dumps({
    "version": os.environ["VERSION"],
    "minimum": os.environ["MINIMUM"] or None,
    "notes_url": "https://github.com/%s/releases/tag/%s" % (os.environ["REPOSITORY"], os.environ["TAG"]),
    "published_unix": int(time.time()),
    "asset": json.loads(os.environ["ASSET"]),
}))' > "$MANIFEST"
gh release upload "$TAG" "$MANIFEST" --clobber --repo "$REPOSITORY"
echo "Announced $VERSION for $PLATFORM"
