#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOST_TARGET="$(rustc -vV | sed -n 's/^host: //p')"
TARGET="${TARGET:-$HOST_TARGET}"
BUILD_TARGET="${BUILD_TARGET:-$TARGET}"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist/desktop}"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/target}"
case "$HOST_TARGET" in
    *-pc-windows-msvc) CARGO_TARGET_DIR="$(cygpath -m "$CARGO_TARGET_DIR")" ;;
esac
export CARGO_TARGET_DIR
cd "$ROOT_DIR"
build_args=(build -p rumux-app -p rumux-cli --release --locked)
binary_dir="$CARGO_TARGET_DIR/release"
if [ "$BUILD_TARGET" != "$HOST_TARGET" ]; then
    build_args+=(--target "$BUILD_TARGET")
    binary_dir="$CARGO_TARGET_DIR/$BUILD_TARGET/release"
fi
if [ -n "${RUMUX_BUILD_FEATURES:-}" ]; then
    build_args+=(--features "$RUMUX_BUILD_FEATURES")
fi
cargo "${build_args[@]}"
python3 "$ROOT_DIR/scripts/package-desktop.py" --target "$TARGET" \
    --binary-dir "$binary_dir" --dist "$DIST_DIR"
