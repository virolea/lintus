#!/bin/sh
# Installs the lintus binary for this machine from its GitHub releases.
#
#   curl -fsSL https://github.com/virolea/lintus/releases/latest/download/install.sh | sh
#
# LINTUS_VERSION picks a release (such as v0.3.0; default: the latest), and
# LINTUS_INSTALL_DIR where the binary goes (default: ~/.local/bin).
set -eu

repo="virolea/lintus"
version="${LINTUS_VERSION:-latest}"
dir="${LINTUS_INSTALL_DIR:-$HOME/.local/bin}"

fail() {
  echo "lintus install: $*" >&2
  exit 1
}

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64 | Linux-amd64) target="x86_64-unknown-linux-musl" ;;
  Linux-aarch64 | Linux-arm64) target="aarch64-unknown-linux-musl" ;;
  Darwin-x86_64) target="x86_64-apple-darwin" ;;
  Darwin-arm64) target="aarch64-apple-darwin" ;;
  *) fail "no prebuilt binary for $(uname -s) $(uname -m). Build it with: cargo install lintus" ;;
esac

case "$version" in
  latest) base="https://github.com/$repo/releases/latest/download" ;;
  v*) base="https://github.com/$repo/releases/download/$version" ;;
  *) base="https://github.com/$repo/releases/download/v$version" ;;
esac

download() {
  if command -v curl > /dev/null; then
    curl -fsSL "$1" -o "$2" || fail "could not download $1"
  elif command -v wget > /dev/null; then
    wget -qO "$2" "$1" || fail "could not download $1"
  else
    fail "curl or wget is needed to download lintus"
  fi
}

sha256() {
  if command -v sha256sum > /dev/null; then
    sha256sum "$1" | cut -d ' ' -f 1
  else
    shasum -a 256 "$1" | cut -d ' ' -f 1
  fi
}

archive="lintus-$target.tar.gz"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

download "$base/$archive" "$tmp/$archive"
download "$base/$archive.sha256" "$tmp/$archive.sha256"
[ "$(cut -d ' ' -f 1 "$tmp/$archive.sha256")" = "$(sha256 "$tmp/$archive")" ] || fail "the checksum of $archive does not match"

tar -xzf "$tmp/$archive" -C "$tmp"
mkdir -p "$dir"
cp "$tmp/lintus" "$dir/lintus"
chmod 755 "$dir/lintus"

echo "Installed $("$dir/lintus" --version) to $dir/lintus"
case ":$PATH:" in
  *":$dir:"*) ;;
  *) echo "$dir is not on your PATH. Add it with: export PATH=\"$dir:\$PATH\"" ;;
esac
