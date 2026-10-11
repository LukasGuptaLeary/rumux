"""Loopback SSH fixture hosted in an explicitly owned WSL1 CI distribution."""
import json
import os
from pathlib import Path, PurePosixPath
import re
import shlex
import subprocess
import tempfile
import uuid

from ssh_fixture import SshFixture, free_port


class WslSshFixture(SshFixture):
    def __init__(self, root):
        super().__init__(root)
        self.distribution = os.environ.get("RUMUX_TEST_WSL_DISTRO", "")
        assert os.name == "nt" and re.fullmatch(r"rumux-ssh-[0-9a-f]{32}", self.distribution), "Set up the owned WSL1 CI fixture first"
        self.remote_root = PurePosixPath("/root") / ("rumux-ssh-" + uuid.uuid4().hex)
        self.remote_python = "python3"

    def remote(self, script, *, check=True):
        # Byte input preserves POSIX LF instead of Windows text-pipe CRLF.
        result = subprocess.run(["wsl.exe", "-d", self.distribution, "-u", "root", "--exec", "/bin/sh", "-s"],
            input=script.encode("utf-8"), capture_output=True, timeout=15)
        result.stdout = result.stdout.decode("utf-8")
        result.stderr = result.stderr.decode("utf-8", errors="replace")
        if check:
            result.check_returncode()
        return result

    def start(self):
        self.root.mkdir()
        self.port = free_port()
        self.directory = self.remote_root / "remote working ' directory"
        self.changed_directory = self.remote_root / "changed directory"
        key = self.root / "client_key"
        subprocess.run(["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(key)], check=True)
        # The real Windows OpenSSH client must accept this private fixture key.
        owner = subprocess.check_output(["whoami", "/user", "/fo", "csv", "/nh"], text=True).strip().rsplit(",", 1)[1].strip('"')
        assert re.fullmatch(r"S-1-[0-9-]+", owner)
        subprocess.run(["icacls", str(key), "/inheritance:r", "/grant:r", "*" + owner + ":F"], check=True, capture_output=True)
        host_key = self.remote_root / "host_key"
        config = self.remote_root / "sshd_config"
        public = (self.root / "client_key.pub").read_text()
        text = (f"Port {self.port}\nListenAddress 127.0.0.1\nHostKey {host_key}\n"
                f"AuthorizedKeysFile {self.remote_root}/authorized_keys\nPidFile {self.remote_root}/sshd.pid\n"
                "PasswordAuthentication no\nKbdInteractiveAuthentication no\nUsePAM no\n"
                "PermitRootLogin prohibit-password\nAllowTcpForwarding local\nLogLevel INFO\n")
        commands = [shlex.join(["mkdir", "-p", str(self.directory), str(self.changed_directory)]),
                    "chmod 700 " + shlex.quote(str(self.remote_root)),
                    "printf %s " + shlex.quote(public) + " > " + str(self.remote_root / "authorized_keys"),
                    "chmod 600 " + str(self.remote_root / "authorized_keys"),
                    shlex.join(["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(host_key)]),
                    "printf %s " + shlex.quote(text) + " > " + str(config)]
        self.remote("set -eu\n" + "\n".join(commands))
        known = self.root / "known hosts"
        known.write_text(f"[127.0.0.1]:{self.port} " + self.remote("cat " + str(host_key) + ".pub").stdout, encoding="utf-8")
        client_config = self.root / "client_config"
        client_config.write_text("", encoding="utf-8")
        self.log = (self.root / "sshd.log").open("w+")
        self.process = subprocess.Popen(["wsl.exe", "-d", self.distribution, "-u", "root", "--exec",
            "/usr/sbin/sshd", "-D", "-e", "-f", str(config)], stdout=self.log, stderr=self.log)
        self.connection = {"host": "127.0.0.1", "user": "root", "port": self.port,
                           "directory": str(self.directory), "identity_file": str(key),
                           "known_hosts_file": str(known), "config_file": str(client_config)}
        return self.listen_and_preview()

    def cleanup(self):
        # Match the exact owned config in /proc before signalling a remote PID.
        directory = str(self.remote_root)
        script = f"""if [ -f {directory}/sshd.pid ]; then
read -r pid < {directory}/sshd.pid
case "$pid" in ''|*[!0-9]*) exit 1;; esac
if tr '\\000' ' ' < /proc/$pid/cmdline | grep -Fq -- {directory}/sshd_config; then kill "$pid"; fi
fi
rm -rf -- {directory}
"""
        try:
            self.remote(script, check=False)
        finally:
            super().cleanup()


if __name__ == "__main__":
    with tempfile.TemporaryDirectory(prefix="rumux-windows-ssh-") as root:
        with WslSshFixture(root) as fixture:
            connection = fixture.connection
            known = connection["known_hosts_file"].replace("\\", "\\\\").replace('"', '\\"')
            result = subprocess.run(["ssh", "-T", "-F", connection["config_file"],
                "-i", connection["identity_file"], "-p", str(connection["port"]),
                "-o", f'UserKnownHostsFile="{known}"', "-o", "StrictHostKeyChecking=yes",
                "--", "root@127.0.0.1", "printf rumux-windows-ssh-fixture"],
                capture_output=True, text=True, timeout=10)
            assert result.returncode == 0 and result.stdout == "rumux-windows-ssh-fixture", (result.stdout, result.stderr)
            print(json.dumps({"native_windows_ssh_fixture": "passed"}))
