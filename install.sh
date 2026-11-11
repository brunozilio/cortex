#!/usr/bin/env bash
# Installs the latest release of this app for this computer:
#
#   curl -fsSL https://raw.githubusercontent.com/{repository}/main/install.sh | bash
#
# {ENV}_VERSION=0.1.0      a given release instead of the latest
# {ENV}_INSTALL_DIR=/path  where the binary goes (~/.local/bin by default)
# ({ENV} is the command in capitals, dashes as underscores.)
#
# Linux gets a static build that runs on any distribution; macOS needs
# Apple silicon. Nothing else has to be installed.

set -euo pipefail

# Written by `cargo run -p xtask -- brand` from brand.toml.
NAME="Lite"
COMMAND="lite"
REPO="brunozilio/cortex"

ENV="$(printf '%s' "$COMMAND" | tr 'a-z-' 'A-Z_')"

# The value of the environment variable {ENV}_$1, or $2.
setting() {
    local name="${ENV}_$1"
    printf '%s' "${!name:-${2:-}}"
}

say() { printf '%s: %s\n' "$COMMAND" "$*" >&2; }
fail() { say "$*"; exit 1; }

target() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"
    case "$arch" in
        x86_64 | amd64) arch="x86_64" ;;
        aarch64 | arm64) arch="aarch64" ;;
        *) fail "no build for the $arch processor" ;;
    esac
    case "$os" in
        Darwin)
            [ "$arch" = "aarch64" ] || fail "no build for Intel Macs; build it with: cargo install --path crates/cli"
            printf 'aarch64-apple-darwin'
            ;;
        Linux) printf '%s-unknown-linux-musl' "$arch" ;;
        *) fail "no build for $os" ;;
    esac
}

latest_version() {
    local url
    url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest")" ||
        fail "could not reach GitHub"
    case "$url" in
        */tag/v*) printf '%s' "${url##*/tag/v}" ;;
        *) fail "no release found" ;;
    esac
}

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

main() {
    command -v curl >/dev/null 2>&1 || fail "curl is needed"
    command -v tar >/dev/null 2>&1 || fail "tar is needed"
    local target version name dir tmp expected actual
    target="$(target)"
    version="$(setting VERSION)"
    version="${version:-$(latest_version)}"
    version="${version#v}"
    name="$COMMAND-$version-$target"
    dir="$(setting INSTALL_DIR "$HOME/.local/bin")"
    tmp="$(mktemp -d)"
    # shellcheck disable=SC2064 # the folder is known now; main's locals are gone at exit
    trap "rm -rf '$tmp'" EXIT

    say "downloading $name"
    local base="https://github.com/$REPO/releases/download/v$version"
    curl -fsSL "$base/$name.tar.gz" -o "$tmp/$name.tar.gz" || fail "$name.tar.gz is not published"
    curl -fsSL "$base/$name.tar.gz.sha256" -o "$tmp/$name.tar.gz.sha256" || fail "$name.tar.gz.sha256 is not published"
    expected="$(awk '{print $1}' "$tmp/$name.tar.gz.sha256")"
    actual="$(sha256 "$tmp/$name.tar.gz")"
    [ "$expected" = "$actual" ] || fail "the download does not match its checksum; nothing was installed"

    tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
    mkdir -p "$dir"
    install -m 755 "$tmp/$name/$COMMAND" "$dir/$COMMAND.new"
    mv -f "$dir/$COMMAND.new" "$dir/$COMMAND"
    say "installed $NAME ($("$dir/$COMMAND" --version)) in $dir"

    case ":$PATH:" in
        *":$dir:"*) say "run: $COMMAND login, then $COMMAND" ;;
        *)
            say "$dir is not on your PATH; add it, e.g.:"
            say "  echo 'export PATH=\"$dir:\$PATH\"' >> ~/.bashrc && source ~/.bashrc"
            ;;
    esac
}

main "$@"
