#!/usr/bin/env python3
"""Verify, extract, install and exercise a real desktop archive in owned paths."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import plistlib
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile


ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--ssh", action="store_true")
    args = parser.parse_args()
    archive = args.archive.resolve()
    checksums = dict(line.split("  ", 1)[::-1] for line in
                     (archive.parent / "rumux-desktop-checksums.txt").read_text().splitlines())
    with archive.open("rb") as stream:
        checksum = hashlib.file_digest(stream, "sha256").hexdigest()
    assert checksum == checksums[archive.name], "Archive checksum mismatch"
    with tempfile.TemporaryDirectory(prefix="rumux-package-") as directory:
        root = Path(directory)
        extracted = root / "extracted"
        extracted.mkdir()
        if sys.platform == "darwin":
            subprocess.run(["ditto", "-x", "-k", str(archive), str(extracted)], check=True)
        elif archive.suffix == ".zip":
            with zipfile.ZipFile(archive) as bundle:
                bundle.extractall(extracted)
        else:
            with tarfile.open(archive) as bundle:
                # Support Python 3.11 builds predating tarfile's data filter.
                # Packages contain only directories and ordinary files.
                for member in bundle.getmembers():
                    path = PurePosixPath(member.name)
                    assert not path.is_absolute() and ".." not in path.parts
                    assert member.isdir() or member.isfile(), "Unexpected archive link/device"
                bundle.extractall(extracted)
        packages = list(extracted.iterdir())
        assert len(packages) == 1 and packages[0].is_dir(), "Expected one package directory"
        package = packages[0]
        meta = json.loads((package / "package.json").read_text())
        assert package.name == "rumux-desktop-" + meta["target"]
        # Spaces in an install path must work for shell, bundle and CLI launch.
        prefix = root / "install with spaces"
        if os.name == "nt":
            install = ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
                       str(package / "install-desktop.ps1"), "-InstallDir", str(prefix), "-NoPath", "-NoShortcut"]
        else:
            install = ["sh", str(package / "install-desktop.sh"), "--prefix", str(prefix)]
        # Exercise the actual installer, including rejection before any install.
        readme = package / "README.txt"
        original = readme.read_bytes()
        readme.write_bytes(original + b"tampered")
        # Windows PowerShell rebuilds its native module path when Python was
        # launched by pwsh; otherwise Get-FileHash resolves incompatible DLLs.
        install_env = {key: value for key, value in os.environ.items()
                       if key.upper() != "PSMODULEPATH"} if os.name == "nt" else None
        rejected = subprocess.run(install, env=install_env, capture_output=True, text=True)
        assert rejected.returncode != 0 and not prefix.exists(), "Installer accepted a damaged package"
        readme.write_bytes(original)
        subprocess.run(install, env=install_env, check=True)
        extension = ".exe" if os.name == "nt" else ""
        app = prefix / "bin" / ("rumux-app" + extension)
        cli = prefix / "bin" / ("rumux" + extension)
        if sys.platform == "darwin":
            contents = prefix / "Applications/Rumux.app/Contents"
            info = plistlib.loads((contents / "Info.plist").read_bytes())
            assert info["CFBundleExecutable"] == "rumux-app"
            assert info["CFBundleIdentifier"] == "io.github.lukasguptaleary.rumux"
            assert (contents / "Resources/AppIcon.icns").is_file()
            assert app.resolve() == (contents / "MacOS/rumux-app").resolve()
        elif os.name != "nt":
            desktop = prefix / "share/applications/io.github.lukasguptaleary.rumux.desktop"
            assert 'Exec="' + str(app.resolve()) + '"' in desktop.read_text()
            validator = shutil.which("desktop-file-validate")
            if validator:
                subprocess.run([validator, str(desktop)], check=True)
        version = subprocess.check_output([str(cli), "version"], text=True).strip()
        assert meta["version"] in version
        command = [sys.executable, str(ROOT / "scripts/smoke-desktop.py"),
                   "--app", str(app), "--cli", str(cli), "--browser"]
        if args.ssh:
            command.append("--ssh")
        subprocess.run(command, check=True)
        if os.name != "nt":
            subprocess.run(command + ["--tcp"], check=True)
        print(json.dumps({"archive": archive.name, "sha256": checksum,
                          "package": meta, "installed_version": version,
                          "passed": ["archive checksum", "tampered-file rejection",
                                     "per-user install with spaces", "native metadata",
                                     "installed CLI", "installed native desktop and service"]}, indent=2))


if __name__ == "__main__":
    main()
