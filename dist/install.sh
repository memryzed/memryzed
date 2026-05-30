#!/usr/bin/env bash
# Memryzed installer for macOS, Linux, and WSL.
#
# Usage:
#   curl -fsSL https://memryzed.com/install.sh | bash
#
# This script detects your OS and CPU architecture, downloads the
# matching release archive from memryzed.com, verifies its SHA-256
# checksum, installs the binary to ~/.memryzed/bin/memryzed, and adds
# that directory to your shell PATH.
#
# Environment overrides:
#   MEMRYZED_VERSION       install a specific version instead of latest
#   MEMRYZED_INSTALL_DIR   install location (default ~/.memryzed/bin)
#   MEMRYZED_ALLOW_ROOT    set to 1 to permit running as root

set -euo pipefail

BASE_URL="https://memryzed.com/releases"
BIN_NAME="memryzed"
INSTALL_DIR="${MEMRYZED_INSTALL_DIR:-$HOME/.memryzed/bin}"

err() { printf 'error: %s\n' "$1" >&2; exit 1; }
info() { printf '%s\n' "$1"; }

if [ "$(id -u)" = "0" ] && [ "${MEMRYZED_ALLOW_ROOT:-0}" != "1" ]; then
  err "refusing to run as root; set MEMRYZED_ALLOW_ROOT=1 to override"
fi

for tool in curl uname tar; do
  command -v "$tool" >/dev/null 2>&1 || err "required tool not found: $tool"
done
command -v sha256sum >/dev/null 2>&1 || command -v shasum >/dev/null 2>&1 \
  || err "need sha256sum or shasum to verify the download"

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Darwin) os_part="apple-darwin" ;;
  Linux)  os_part="unknown-linux-gnu" ;;
  *) err "unsupported OS: $os" ;;
esac

case "$arch" in
  x86_64|amd64) arch_part="x86_64" ;;
  arm64|aarch64) arch_part="aarch64" ;;
  *) err "unsupported architecture: $arch" ;;
esac

target="${arch_part}-${os_part}"

# Resolve the version. memryzed.com/releases/latest.txt holds the
# current version string (for example "0.5.0").
if [ -n "${MEMRYZED_VERSION:-}" ]; then
  version="$MEMRYZED_VERSION"
else
  info "Resolving latest version..."
  version="$(curl -fsSL "${BASE_URL}/latest.txt" | tr -d '[:space:]')"
  [ -n "$version" ] || err "could not determine the latest version"
fi

archive="memryzed-${target}.tar.gz"
base="${BASE_URL}/v${version}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

info "Downloading ${archive} (v${version})..."
curl -fsSL "${base}/${archive}" -o "${tmp}/${archive}" \
  || err "download failed; no asset for ${target} in v${version}"
curl -fsSL "${base}/${archive}.sha256" -o "${tmp}/${archive}.sha256" \
  || err "checksum download failed"

info "Verifying checksum..."
expected="$(awk '{print $1}' "${tmp}/${archive}.sha256")"
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "${tmp}/${archive}" | awk '{print $1}')"
else
  actual="$(shasum -a 256 "${tmp}/${archive}" | awk '{print $1}')"
fi
[ "$expected" = "$actual" ] || err "checksum mismatch (expected $expected, got $actual)"

info "Extracting..."
tar xzf "${tmp}/${archive}" -C "$tmp"

mkdir -p "$INSTALL_DIR" || err "cannot create $INSTALL_DIR"
cp "${tmp}/memryzed-${target}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"
chmod +x "${INSTALL_DIR}/${BIN_NAME}"

add_path() {
  rc="$1"
  [ -f "$rc" ] || return 0
  if ! grep -q '.memryzed/bin' "$rc" 2>/dev/null; then
    printf '\n# Memryzed\nexport PATH="%s:$PATH"\n' "$INSTALL_DIR" >> "$rc"
    info "Added $INSTALL_DIR to PATH in $rc"
  fi
}
add_path "$HOME/.bashrc"
add_path "$HOME/.zshrc"
[ -f "$HOME/.config/fish/config.fish" ] && \
  ! grep -q '.memryzed/bin' "$HOME/.config/fish/config.fish" 2>/dev/null && \
  printf '\n# Memryzed\nfish_add_path %s\n' "$INSTALL_DIR" >> "$HOME/.config/fish/config.fish"

info ""
info "Memryzed v${version} installed to ${INSTALL_DIR}/${BIN_NAME}"
info ""
info "Next:"
info "  1. Restart your shell, or run: export PATH=\"${INSTALL_DIR}:\$PATH\""
info "  2. Initialize:                 memryzed init"
info "  3. Wire into your agent:       memryzed install"
