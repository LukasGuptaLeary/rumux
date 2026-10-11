#!/usr/bin/env python3
"""Stage native desktop + CLI binaries, installers and checksummed archives."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import tarfile
import tomllib
import zipfile


ROOT = Path(__file__).resolve().parents[1]
BUNDLE_ID = "io.github.lukasguptaleary.rumux"


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def copy(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)


def package(target, binaries, dist):
    if target.endswith("-unknown-linux-gnu"):
        platform = "linux"
    elif target.endswith("-apple-darwin"):
        platform = "macos"
    elif target.endswith("-pc-windows-msvc"):
        platform = "windows"
    else:
        raise ValueError(f"Unsupported desktop packaging target: {target}")
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    parent = dist / "package" / target
    # Remove only this script's prior staging directory, never install/config paths.
    if parent.exists():
        shutil.rmtree(parent)
    stage = parent / f"rumux-desktop-{target}"
    stage.mkdir(parents=True)
    artifacts = dist / "artifacts"
    artifacts.mkdir(parents=True, exist_ok=True)
    extension = ".exe" if platform == "windows" else ""
    bin_dir = stage / "bin"
    if platform == "macos":
        contents = stage / "Rumux.app" / "Contents"
        bin_dir = contents / "MacOS"
        resources = contents / "Resources"
        for name in ("rumux-icon-1024.png", "rumux-icon.svg"):
            copy(ROOT / "assets" / "brand" / name, resources / name)
        iconset = parent / "AppIcon.iconset"
        iconset.mkdir()
        for logical, scale in ((16, 1), (16, 2), (32, 1), (32, 2), (128, 1), (128, 2), (256, 1), (256, 2), (512, 1), (512, 2)):
            suffix = "@2x" if scale == 2 else ""
            subprocess.run(["sips", "-z", str(logical * scale), str(logical * scale),
                            str(ROOT / "assets/brand/rumux-icon-1024.png"), "--out",
                            str(iconset / f"icon_{logical}x{logical}{suffix}.png")], check=True, stdout=subprocess.DEVNULL)
        subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(resources / "AppIcon.icns")], check=True)
        shutil.rmtree(iconset)
        info = {"CFBundleDevelopmentRegion": "en", "CFBundleDisplayName": "Rumux",
                "CFBundleExecutable": "rumux-app", "CFBundleIconFile": "AppIcon",
                "CFBundleIdentifier": BUNDLE_ID, "CFBundleInfoDictionaryVersion": "6.0",
                "CFBundleName": "Rumux", "CFBundlePackageType": "APPL",
                "CFBundleShortVersionString": version, "CFBundleVersion": version,
                "LSApplicationCategoryType": "public.app-category.developer-tools",
                "LSMinimumSystemVersion": "14.0", "NSHighResolutionCapable": True}
        (contents / "Info.plist").write_bytes(plistlib.dumps(info))
    for name in ("rumux-app", "rumux"):
        copy(binaries / (name + extension), bin_dir / (name + extension))
    if platform == "linux":
        for name in (BUNDLE_ID + ".desktop", BUNDLE_ID + ".metainfo.xml"):
            destination = "applications" if name.endswith(".desktop") else "metainfo"
            copy(ROOT / "assets/linux" / name, stage / "share" / destination / name)
        copy(ROOT / "assets/brand/rumux-icon-1024.png", stage / f"share/icons/hicolor/512x512/apps/{BUNDLE_ID}.png")
        copy(ROOT / "assets/brand/rumux-icon.svg", stage / f"share/icons/hicolor/scalable/apps/{BUNDLE_ID}.svg")
    installer = "install-desktop.ps1" if platform == "windows" else "install-desktop.sh"
    copy(ROOT / "scripts" / installer, stage / installer)
    for source, name in (("LICENSE", "rumux-MIT"),
                         ("crates/gpui/LICENSE-APACHE", "gpui-APACHE"),
                         ("crates/gpui-component/LICENSE-APACHE", "gpui-component-APACHE"),
                         ("crates/gpui-terminal/LICENSE-MIT", "gpui-terminal-MIT"),
                         ("crates/gpui-terminal/LICENSE-APACHE", "gpui-terminal-APACHE"),
                         ("crates/alacritty_terminal/LICENSE-APACHE", "alacritty-terminal-APACHE")):
        copy(ROOT / source, stage / "licenses" / (name + ".txt"))
    commit = os.environ.get("RUMUX_SOURCE_COMMIT")
    if commit is None:
        commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        dirty = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT))
    else:
        assert len(commit) == 40 and all(ch in "0123456789abcdef" for ch in commit)
        assert os.environ.get("RUMUX_SOURCE_DIRTY") in ("true", "false")
        dirty = os.environ["RUMUX_SOURCE_DIRTY"] == "true"
    (stage / "package.json").write_text(json.dumps({"version": version, "target": target,
        "source_commit": commit, "source_dirty": dirty,
        "build_features": os.environ.get("RUMUX_BUILD_FEATURES", ""),
        "source_archive_sha256": os.environ.get("RUMUX_SOURCE_ARCHIVE_SHA256")}, indent=2) + "\n")
    command = "powershell -ExecutionPolicy Bypass -File .\\install-desktop.ps1" if platform == "windows" else "sh ./install-desktop.sh"
    runtime = {"linux": "Requires X11/XWayland, GTK 3, WebKitGTK 4.1 and the GPUI graphics libraries. Native Wayland supports terminals only.",
               "macos": "Requires macOS 14 or later. This bundle is unsigned and not notarized.",
               "windows": "Requires Windows 10/11 x64, DirectX 11 and Microsoft Edge WebView2 Runtime. This package is unsigned."}[platform]
    (stage / "README.txt").write_text(f"Rumux {version} ({target})\n\nInstall for the current user:\n  {command}\n"
        "The installer checks files.sha256, installs the desktop and CLI, and preserves configuration.\n"
        "Close the desktop and run rumux session stop before upgrading running binaries.\n"
        f"\n{runtime}\n\nTerminal processes survive desktop exit. rumux session stop stops them explicitly.\n"
        "History is private but may contain printed secrets; persist_scrollback=false disables disk history for new panes.\n")
    (stage / "files.sha256").write_text("".join(f"{digest(path)}  {path.relative_to(stage).as_posix()}\n"
        for path in sorted(stage.rglob("*")) if path.is_file()))
    archive = artifacts / f"rumux-desktop-{target}.{'tar.gz' if platform == 'linux' else 'zip'}"
    if platform == "linux":
        with tarfile.open(archive, "w:gz") as bundle:
            bundle.add(stage, arcname=stage.name)
    else:
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
            for path in sorted(stage.rglob("*")):
                if path.is_file():
                    bundle.write(path, path.relative_to(parent).as_posix())
    archives = sorted(p for p in artifacts.glob("rumux-desktop-*") if p.suffix in (".zip", ".gz"))
    (artifacts / "rumux-desktop-checksums.txt").write_text("".join(f"{digest(path)}  {path.name}\n" for path in archives))
    print(json.dumps({"archive": str(archive), "sha256": digest(archive), "source_commit": commit, "source_dirty": dirty}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True)
    parser.add_argument("--binary-dir", type=Path, required=True)
    parser.add_argument("--dist", type=Path, default=ROOT / "dist/desktop")
    args = parser.parse_args()
    package(args.target, args.binary_dir.resolve(), args.dist.resolve())
