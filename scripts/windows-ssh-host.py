#!/usr/bin/env python3
"""Create/remove one owned WSL1 SSH target on a disposable GitHub Windows runner.

The Windows Rumux binaries remain native. WSL supplies only the POSIX remote
host; its loopback networking also reaches the Windows preview fixture.
"""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import urllib.request
import uuid


URL = "https://cloud-images.ubuntu.com/wsl/jammy/current/ubuntu-jammy-wsl-amd64-ubuntu22.04lts.rootfs.tar.gz"
# Canonical's published SHA256SUMS, checked 2026-10-11. Updates must change
# both this digest and the cache key; never import an unverified rootfs.
SHA256 = "1483cc5c1dce13064f774834cbffdff226559fd522a67a381a8ea77d63fb4109"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["setup", "cleanup"])
    args = parser.parse_args()
    assert os.name == "nt" and os.environ.get("GITHUB_ACTIONS") == "true", "Disposable Windows CI only"
    temporary = Path(os.environ["RUNNER_TEMP"])
    state = temporary / "rumux-owned-wsl.json"
    if args.action == "cleanup":
        if state.exists():
            data = json.loads(state.read_text())
            assert re.fullmatch(r"rumux-ssh-[0-9a-f]{32}", data["name"])
            root = Path(data["root"]).resolve()
            assert root.parent == temporary.resolve() and root.name.startswith("rumux-wsl-")
            subprocess.run(["wsl.exe", "--unregister", data["name"]], check=data["imported"], timeout=60)
            state.unlink()
            shutil.rmtree(root)
        return
    assert not state.exists(), "An owned WSL fixture already exists"
    cache = temporary / "rumux-wsl-rootfs"
    cache.mkdir(exist_ok=True)
    compressed = cache / (SHA256 + ".tar.gz")
    if not compressed.exists():
        with urllib.request.urlopen(URL, timeout=60) as source, compressed.open("wb") as target:
            shutil.copyfileobj(source, target)
    with compressed.open("rb") as stream:
        assert hashlib.file_digest(stream, "sha256").hexdigest() == SHA256, "WSL rootfs checksum mismatch"
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
        output.write("rootfs_verified=true\n")
    root = Path(tempfile.mkdtemp(prefix="rumux-wsl-", dir=temporary))
    name = "rumux-ssh-" + uuid.uuid4().hex
    archive = root / "rootfs.tar"
    with gzip.open(compressed, "rb") as source, archive.open("wb") as target:
        shutil.copyfileobj(source, target)
    data = {"name": name, "root": str(root), "imported": False}
    state.write_text(json.dumps(data))
    subprocess.run(["wsl.exe", "--import", name, str(root / "distro"), str(archive), "--version", "1"],
                   check=True, timeout=120)
    data["imported"] = True
    state.write_text(json.dumps(data))
    archive.unlink()
    # Suppress package service startup: only the later owned sshd may listen,
    # and its configuration binds strictly to 127.0.0.1.
    script = """set -eu
export DEBIAN_FRONTEND=noninteractive
printf '#!/bin/sh\\nexit 101\\n' > /usr/sbin/policy-rc.d
chmod 755 /usr/sbin/policy-rc.d
apt-get update
apt-get install -y --no-install-recommends openssh-server python3
mkdir -p /run/sshd
chmod 700 /root
usermod -p '' root
ssh -V
python3 --version
"""
    subprocess.run(["wsl.exe", "-d", name, "-u", "root", "--exec", "/bin/sh", "-s"],
                   input=script.encode("utf-8"), check=True, timeout=300)
    with Path(os.environ["GITHUB_ENV"]).open("a", encoding="utf-8") as output:
        output.write("RUMUX_TEST_WSL_DISTRO=" + name + "\n")
    print(json.dumps({"owned_wsl": name, "version": 1, "rootfs_sha256": SHA256}))


if __name__ == "__main__":
    main()
