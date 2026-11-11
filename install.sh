#!/usr/bin/env bash
# Installs the latest release of this app for this computer:
#
#   curl -fsSL https://raw.githubusercontent.com/<repository>/main/install.sh | bash
#
# <ENV>_VERSION=0.6.1      a given release instead of the latest
# <ENV>_INSTALL_DIR=/path  where the binary goes (~/.local/bin by default)
# <ENV>_LIBC=musl|gnu      Linux: force the static (musl) or glibc build
# (<ENV> is the command in capitals, dashes as underscores.)
#
# On Linux the glibc build (with semantic memory) is used when the system's
# glibc is new enough, the static musl build otherwise (RHEL 8/9, older
# Debian/Ubuntu). `<command> update` keeps to the build installed.

set -euo pipefail

# Written by `cargo run -p xtask -- brand` from brand.toml.
NAME="Superfik"
COMMAND="superfik"
REPO="brunozilio/lite"

ENV="$(printf '%s' "$COMMAND" | tr 'a-z-' 'A-Z_')"
# The glibc the published glibc builds need (they are built on Ubuntu 24.04).
MIN_GLIBC="2.39"

# The value of the environment variable <ENV>_$1, or $2.
setting() {
    local name="${ENV}_$1"
    printf '%s' "${!name:-${2:-}}"
}

say() { printf '%s: %s\n' "$COMMAND" "$*" >&2; }
fail() { say "$*"; exit 1; }

# Whether version $1 is at least $2 (both like 2.39).
at_least() {
    local have_major="${1%%.*}" have_minor="${1#*.}" need_major="${2%%.*}" need_minor="${2#*.}"
    have_minor="${have_minor%%.*}"
    [ "$have_major" -gt "$need_major" ] || { [ "$have_major" -eq "$need_major" ] && [ "$have_minor" -ge "$need_minor" ]; }
}

# gnu or musl: the build this Linux can run.
linux_libc() {
    if [ -n "$(setting LIBC)" ]; then
        setting LIBC
        return
    fi
    local glibc
    glibc="$(getconf GNU_LIBC_VERSION 2>/dev/null | awk '{print $2}')" || true
    if [ -n "$glibc" ] && at_least "$glibc" "$MIN_GLIBC"; then
        printf 'gnu'
    else
        printf 'musl'
    fi
}

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
        Linux) printf '%s-unknown-linux-%s' "$arch" "$(linux_libc)" ;;
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
    say "installed $("$dir/$COMMAND" --version) in $dir"
    case "$target" in
        *musl) say "this static build of $NAME has no semantic memory search (memory search is by words)" ;;
    esac

    case ":$PATH:" in
        *":$dir:"*) say "run: $COMMAND login, then $COMMAND" ;;
        *)
            say "$dir is not on your PATH; add it, e.g.:"
            say "  echo 'export PATH=\"$dir:\$PATH\"' >> ~/.bashrc && source ~/.bashrc"
            ;;
    esac
}

main "$@"
