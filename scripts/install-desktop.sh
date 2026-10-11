#!/bin/sh
# Per-user installation from an extracted Rumux desktop archive.
set -eu
SOURCE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
case "$(uname -s)" in
    Darwin) PLATFORM=macos; PREFIX="$HOME"; BIN_DIR="$HOME/.local/bin" ;;
    Linux) PLATFORM=linux; PREFIX="$HOME/.local"; BIN_DIR="$HOME/.local/bin" ;;
    *) echo "Use install-desktop.ps1 on Windows." >&2; exit 1 ;;
esac
while [ $# -gt 0 ]; do
    case "$1" in
        --prefix)
            [ $# -ge 2 ] || { echo "--prefix requires a directory" >&2; exit 1; }
            PREFIX=$2; BIN_DIR="$PREFIX/bin"; shift 2 ;;
        --help|-h)
            echo "Usage: sh install-desktop.sh [--prefix directory]"; exit 0 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done
# Verify before mutating the installation. Config/runtime directories are not used.
(
    cd "$SOURCE"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum -c files.sha256 >/dev/null
    else
        shasum -a 256 -c files.sha256 >/dev/null
    fi
)
mkdir -p "$PREFIX" "$BIN_DIR"
PREFIX=$(CDPATH= cd -- "$PREFIX" && pwd)
BIN_DIR=$(CDPATH= cd -- "$BIN_DIR" && pwd)
if [ "$PLATFORM" = macos ]; then
    mkdir -p "$PREFIX/Applications"
    # ditto preserves executable permissions and native bundle metadata.
    ditto "$SOURCE/Rumux.app" "$PREFIX/Applications/Rumux.app"
    APP="$PREFIX/Applications/Rumux.app/Contents/MacOS/rumux-app"
    CLI="$PREFIX/Applications/Rumux.app/Contents/MacOS/rumux"
    mkdir -p "$PREFIX/Applications/Rumux.app/Contents/Resources/licenses"
    cp "$SOURCE/licenses/"* "$PREFIX/Applications/Rumux.app/Contents/Resources/licenses/"
else
    mkdir -p "$PREFIX/share/rumux/bin" "$PREFIX/share/rumux/licenses"
    cp "$SOURCE/bin/"* "$PREFIX/share/rumux/bin/"
    chmod 755 "$PREFIX/share/rumux/bin/rumux" "$PREFIX/share/rumux/bin/rumux-app"
    cp "$SOURCE/licenses/"* "$PREFIX/share/rumux/licenses/"
    cp -R "$SOURCE/share/." "$PREFIX/share/"
    APP="$PREFIX/share/rumux/bin/rumux-app"
    CLI="$PREFIX/share/rumux/bin/rumux"
    escaped_app=$(printf '%s' "$APP" | sed 's/\\/\\\\/g; s/"/\\"/g; s/\$/\\$/g; s/`/\\`/g')
    desktop="$PREFIX/share/applications/io.github.lukasguptaleary.rumux.desktop"
    sed '/^Exec=/d' "$SOURCE/share/applications/io.github.lukasguptaleary.rumux.desktop" > "$desktop"
    printf 'Exec="%s"\n' "$escaped_app" >> "$desktop"
fi
ln -sfn "$CLI" "$BIN_DIR/rumux"
ln -sfn "$APP" "$BIN_DIR/rumux-app"
printf 'Installed Rumux desktop and CLI in %s\n' "$PREFIX"
case ":$PATH:" in
    *:"$BIN_DIR":*) ;;
    *) printf 'Add %s to your PATH to use rumux from a shell.\n' "$BIN_DIR" ;;
esac
