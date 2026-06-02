#!/bin/sh
# meditate installer (macOS / Linux). Downloads the latest release binary,
# verifies its checksum, and installs it to ~/.local/bin.
set -eu

REPO="momentmaker/meditate-cli"
BIN="meditate"

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
    Darwin) os_id="apple-darwin" ;;
    Linux) os_id="unknown-linux-gnu" ;;
    *) echo "meditate: unsupported OS '$os' — on Windows use install.ps1" >&2; exit 1 ;;
esac
case "$arch" in
    x86_64 | amd64) arch_id="x86_64" ;;
    arm64 | aarch64) arch_id="aarch64" ;;
    *) echo "meditate: unsupported architecture '$arch'" >&2; exit 1 ;;
esac
target="${arch_id}-${os_id}"

auth=""
if [ -n "${GITHUB_TOKEN:-}" ]; then
    auth="-H Authorization:Bearer ${GITHUB_TOKEN}"
fi
tag="$(curl -fsSL $auth "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | head -1 | cut -d '"' -f4)"
if [ -z "$tag" ]; then
    echo "meditate: could not find the latest release (GitHub rate limit? set GITHUB_TOKEN)" >&2
    exit 1
fi

base="https://github.com/${REPO}/releases/download/${tag}"
archive="${BIN}-${target}.tar.gz"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

curl -fsSL "${base}/${archive}" -o "${tmp}/${archive}"
curl -fsSL "${base}/checksums.txt" -o "${tmp}/checksums.txt"

expected="$(grep " ${archive}\$" "${tmp}/checksums.txt" | awk '{print $1}')"
actual="$(sha256 "${tmp}/${archive}")"
if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
    echo "meditate: checksum verification failed — aborting" >&2
    exit 1
fi

tar -xzf "${tmp}/${archive}" -C "$tmp"
dest="${HOME}/.local/bin"
mkdir -p "$dest"
install -m 0755 "${tmp}/${BIN}" "${dest}/${BIN}"

echo "Installed ${BIN} ${tag} to ${dest}/${BIN}"
case ":$PATH:" in
    *":$dest:"*) ;;
    *) echo "Add ${dest} to your PATH to run 'meditate'." ;;
esac
