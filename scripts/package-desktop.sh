#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
VERSION="${VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -n1)}"
APP_NAME="rumux"
BIN_NAME="rumux-app"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist/desktop}"
BUILD_TARGET="${BUILD_TARGET:-$TARGET}"
PACKAGE_DIR="$DIST_DIR/package/$TARGET"
ARTIFACT_DIR="$DIST_DIR/artifacts"

checksum_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$@"
    else
        shasum -a 256 "$@"
    fi
}

rm -rf "$PACKAGE_DIR"
mkdir -p "$PACKAGE_DIR" "$ARTIFACT_DIR"

build_binary() {
    cargo build -p rumux-app --release --locked --target "$BUILD_TARGET"
}

package_linux() {
    local archive_root="$PACKAGE_DIR/${APP_NAME}-linux-${TARGET}"
    mkdir -p "$archive_root/bin"

    cp "$ROOT_DIR/target/$BUILD_TARGET/release/$BIN_NAME" "$archive_root/bin/$BIN_NAME"

    cat > "$archive_root/README.txt" <<EOF
rumux desktop app
Version: $VERSION
Target: $TARGET

Run:
  ./bin/$BIN_NAME

Linux runtime notes:
- GPUI requires the usual X11/Wayland runtime libraries for your distro.
- If you are packaging for a distro, prefer wrapping this binary in a native package later.
EOF

    tar -C "$PACKAGE_DIR" -czf "$ARTIFACT_DIR/${APP_NAME}-desktop-${TARGET}.tar.gz" "$(basename "$archive_root")"
}

package_macos() {
    local bundle_root="$PACKAGE_DIR/${APP_NAME}.app"
    local contents="$bundle_root/Contents"
    local macos_dir="$contents/MacOS"
    local resources_dir="$contents/Resources"

    mkdir -p "$macos_dir" "$resources_dir"
    cp "$ROOT_DIR/target/$BUILD_TARGET/release/$BIN_NAME" "$macos_dir/$APP_NAME"

    cat > "$contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>rumux</string>
  <key>CFBundleExecutable</key>
  <string>rumux</string>
  <key>CFBundleIdentifier</key>
  <string>io.github.lukasguptaleary.rumux</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>rumux</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>$VERSION</string>
  <key>CFBundleVersion</key>
  <string>$VERSION</string>
  <key>LSMinimumSystemVersion</key>
  <string>14.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF

    ditto -c -k --sequesterRsrc --keepParent "$bundle_root" "$ARTIFACT_DIR/${APP_NAME}-desktop-${TARGET}.zip"
}

package_windows() {
    local archive_root="$PACKAGE_DIR/${APP_NAME}-desktop-${TARGET}"
    mkdir -p "$archive_root"

    cp "$ROOT_DIR/target/$BUILD_TARGET/release/$BIN_NAME.exe" "$archive_root/$BIN_NAME.exe"

    cat > "$archive_root/README.txt" <<EOF
rumux desktop app
Version: $VERSION
Target: $TARGET

Run:
  .\\$BIN_NAME.exe

Windows notes:
- This is a raw desktop binary package, not an MSI installer.
- Native packaging, signing, and installer UX can be added later once Windows support is validated.
EOF

    (
        cd "$PACKAGE_DIR"
        zip -q -r "$ARTIFACT_DIR/${APP_NAME}-desktop-${TARGET}.zip" "$(basename "$archive_root")"
    )
}

case "$TARGET" in
    *-unknown-linux-gnu)
        build_binary
        package_linux
        ;;
    *-apple-darwin)
        build_binary
        package_macos
        ;;
    *-pc-windows-msvc)
        build_binary
        package_windows
        ;;
    *)
        echo "unsupported desktop packaging target: $TARGET" >&2
        exit 1
        ;;
esac

(
    cd "$ARTIFACT_DIR"
    checksum_file rumux-desktop-* > rumux-desktop-checksums.txt
)
