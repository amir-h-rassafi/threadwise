#!/usr/bin/env sh
# Threadwise installer: downloads the prebuilt tw binary from a GitHub release
# and drops it on your PATH.
#
# Usage:
#   curl -sSfL https://raw.githubusercontent.com/amir-h-rassafi/threadwise/main/scripts/install.sh | sh
#   PREFIX=$HOME/.local sh install.sh         # user install, no sudo
#   TW_VERSION=v0.1.1 sh install.sh           # pin a version (default: latest)

set -eu

REPO="amir-h-rassafi/threadwise"
TAG="${TW_VERSION:-latest}"
PREFIX="${PREFIX:-/usr/local}"
BIN_DIR="${PREFIX}/bin"

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Darwin) os_tag="darwin" ;;
  Linux)  os_tag="linux"  ;;
  *) echo "threadwise: unsupported OS: $os" >&2; exit 1 ;;
esac

case "$arch" in
  x86_64|amd64)  arch_tag="amd64" ;;
  aarch64|arm64) arch_tag="arm64" ;;
  *) echo "threadwise: unsupported architecture: $arch" >&2; exit 1 ;;
esac

asset="threadwise-${os_tag}-${arch_tag}.tar.gz"
if [ "$TAG" = "latest" ]; then
  url="https://github.com/${REPO}/releases/latest/download/${asset}"
else
  url="https://github.com/${REPO}/releases/download/${TAG}/${asset}"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "threadwise: downloading $url"
curl -fsSL "$url" | tar -xz -C "$tmp"

bin_src="$tmp/threadwise-${os_tag}-${arch_tag}/tw"
if [ ! -x "$bin_src" ]; then
  echo "threadwise: tw binary not found in archive" >&2
  exit 1
fi

mkdir -p "$BIN_DIR" 2>/dev/null || true

if [ -w "$BIN_DIR" ] || [ "$(id -u)" = "0" ]; then
  install -m 0755 "$bin_src" "$BIN_DIR/tw"
else
  echo "threadwise: $BIN_DIR is not writable; retrying with sudo"
  sudo install -m 0755 "$bin_src" "$BIN_DIR/tw"
fi

echo "threadwise: installed $BIN_DIR/tw"
"$BIN_DIR/tw" --version
