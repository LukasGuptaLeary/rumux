#!/bin/sh

set -eu

REPO="LukasGuptaLeary/rumux"
VERSION="${RUMUX_VERSION:-latest}"
INSTALL_DIR="${RUMUX_INSTALL_DIR:-${HOME}/.local/bin}"

usage() {
    cat <<'EOF'
Usage: install.sh [--version <tag>] [--dir <path>]

Options:
  --version <tag>  Install a specific release tag such as v0.1.0
  --dir <path>     Install into a custom directory
  --help           Show this help text

Environment:
  RUMUX_VERSION      Alternative way to set the release tag
  RUMUX_INSTALL_DIR  Alternative way to set the install directory
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --version)
            VERSION="$2"
            shift 2
            ;;
        --dir|--bin-dir)
            INSTALL_DIR="$2"
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

normalize_version() {
    case "$1" in
        latest) echo "latest" ;;
        v*) echo "$1" ;;
        *) echo "v$1" ;;
    esac
}

sha256_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    elif command -v openssl >/dev/null 2>&1; then
        openssl dgst -sha256 "$1" | awk '{print $NF}'
    else
        echo "No SHA-256 implementation found (need sha256sum, shasum, or openssl)." >&2
        exit 1
    fi
}

download() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$1" -o "$2"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$2" "$1"
    else
        echo "curl or wget is required to install rumux." >&2
        exit 1
    fi
}

VERSION="$(normalize_version "$VERSION")"

uname_s="$(uname -s)"
uname_m="$(uname -m)"

case "$uname_s" in
    Darwin) os_part="apple-darwin" ;;
    Linux) os_part="unknown-linux-gnu" ;;
    *)
        echo "Unsupported operating system: $uname_s" >&2
        exit 1
        ;;
esac

case "$uname_m" in
    x86_64|amd64) arch_part="x86_64" ;;
    arm64|aarch64) arch_part="aarch64" ;;
    *)
        echo "Unsupported architecture: $uname_m" >&2
        exit 1
        ;;
esac

target="${arch_part}-${os_part}"

case "$target" in
    x86_64-unknown-linux-gnu|aarch64-apple-darwin) ;;
    *)
        echo "No prebuilt installer asset is published for $target yet." >&2
        echo "Use a source install instead: cargo install --git https://github.com/${REPO}.git rumux-cli --bin rumux" >&2
        exit 1
        ;;
esac

archive_name="rumux-${target}.tar.gz"
checksums_name="rumux-checksums.txt"

if [ "$VERSION" = "latest" ]; then
    base_url="https://github.com/${REPO}/releases/latest/download"
else
    base_url="https://github.com/${REPO}/releases/download/${VERSION}"
fi

tmp_dir="$(mktemp -d 2>/dev/null || mktemp -d -t rumux-install)"
trap 'rm -rf "$tmp_dir"' EXIT INT TERM

archive_path="${tmp_dir}/${archive_name}"
checksums_path="${tmp_dir}/${checksums_name}"

download "${base_url}/${archive_name}" "$archive_path"
download "${base_url}/${checksums_name}" "$checksums_path"

expected_checksum="$(awk "/  ${archive_name}\$/{print \$1}" "$checksums_path")"

if [ -z "$expected_checksum" ]; then
    echo "Could not find checksum for ${archive_name}." >&2
    exit 1
fi

actual_checksum="$(sha256_file "$archive_path")"

if [ "$expected_checksum" != "$actual_checksum" ]; then
    echo "Checksum verification failed for ${archive_name}." >&2
    exit 1
fi

mkdir -p "$INSTALL_DIR"
tar -xzf "$archive_path" -C "$tmp_dir"

if command -v install >/dev/null 2>&1; then
    install -m 755 "${tmp_dir}/rumux" "${INSTALL_DIR}/rumux"
else
    cp "${tmp_dir}/rumux" "${INSTALL_DIR}/rumux"
    chmod 755 "${INSTALL_DIR}/rumux"
fi

echo "Installed rumux to ${INSTALL_DIR}/rumux"

case ":$PATH:" in
    *:"${INSTALL_DIR}":*) ;;
    *)
        echo "Add ${INSTALL_DIR} to your PATH to run rumux globally." >&2
        ;;
esac
