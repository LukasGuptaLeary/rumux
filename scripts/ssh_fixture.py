"""Temporary loopback SSH server for the native desktop integration suite.

All host/client keys, authorized keys, configuration and web content are local
fixtures. This never changes the machine's SSH service or user's known_hosts.
"""
import getpass
import http.server
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time


def free_port():
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


class SshFixture:
    def __init__(self, root):
        self.root = Path(root) / "ssh"
        self.process = None
        self.server = None
        self.thread = None
        self.log = None
        self.auth_directory = None

    def __enter__(self):
        try:
            return self.start()
        except BaseException:
            self.cleanup()
            raise

    def start(self):
        assert os.name != "nt", "The SSH server fixture requires a POSIX account shell"
        self.root.mkdir(mode=0o700)
        self.directory = self.root / "remote working ' directory"
        self.changed_directory = self.root / "changed directory"
        self.directory.mkdir()
        self.changed_directory.mkdir()
        sshd = shutil.which("sshd") or "/usr/sbin/sshd"
        assert Path(sshd).is_file(), "Install OpenSSH server to run --ssh"
        self.port = free_port()
        for key in ["host_key", "client_key"]:
            subprocess.run(["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(self.root / key)], check=True)
        # sshd StrictModes checks every ancestor of AuthorizedKeysFile.
        # /tmp is world-writable on Linux, so keep this disposable key file
        # beneath the account's home without touching its .ssh directory.
        self.auth_directory = tempfile.TemporaryDirectory(prefix=".rumux-ssh-test-", dir=Path.home())
        authorized = Path(self.auth_directory.name) / "authorized_keys"
        authorized.write_bytes((self.root / "client_key.pub").read_bytes())
        authorized.chmod(0o600)
        known = self.root / "known hosts"
        known.write_text(f"[127.0.0.1]:{self.port} " + (self.root / "host_key.pub").read_text(), encoding="utf-8")
        client_config = self.root / "client_config"
        client_config.write_text("", encoding="utf-8")
        config = self.root / "sshd_config"
        config.write_text(
            f"Port {self.port}\nListenAddress 127.0.0.1\nHostKey {self.root / 'host_key'}\n"
            f"AuthorizedKeysFile {json.dumps(str(authorized))}\nPidFile {self.root / 'sshd.pid'}\n"
            "PasswordAuthentication no\nKbdInteractiveAuthentication no\n"
            + ("UsePAM yes\n" if sys.platform.startswith("linux") else "UsePAM no\n")
            + "PermitRootLogin prohibit-password\nAllowTcpForwarding local\nLogLevel INFO\n",
            encoding="utf-8")
        self.log = (self.root / "sshd.log").open("w+")
        self.process = subprocess.Popen([sshd, "-D", "-e", "-f", str(config)], stdout=self.log, stderr=self.log)
        self.connection = {"host":"127.0.0.1", "user":getpass.getuser(), "port":self.port,
                           "directory":str(self.directory), "identity_file":str(self.root / "client_key"),
                           "known_hosts_file":str(known), "config_file":str(client_config)}
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                self.cleanup()
                raise RuntimeError((self.root / "sshd.log").read_text())
            try:
                with socket.create_connection(("127.0.0.1", self.port), timeout=.1):
                    break
            except OSError:
                time.sleep(.05)
        else:
            self.cleanup()
            raise RuntimeError("SSH fixture did not listen")

        class Preview(http.server.BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass
            def do_GET(self):
                body = b"<!doctype html><title>SSH forwarded preview</title><h1>Remote preview through SSH</h1>"
                self.send_response(200)
                self.send_header("Content-Type", "text/html; charset=utf-8")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Preview)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.local_port = free_port()
        self.connection["forwards"] = [{"local_port":self.local_port,"remote_port":self.server.server_port}]
        return self

    def cleanup(self):
        if self.server is not None:
            self.server.shutdown()
            self.server.server_close()
        if self.thread is not None:
            self.thread.join(timeout=5)
        if self.process is not None and self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)
        if self.log is not None:
            self.log.close()
        if self.auth_directory is not None:
            self.auth_directory.cleanup()

    def __exit__(self, *args):
        self.cleanup()


if __name__ == "__main__":
    import tempfile
    with tempfile.TemporaryDirectory(prefix="rumux-ssh-fixture-") as root:
        with SshFixture(root) as fixture:
            connection = fixture.connection
            result = subprocess.run(["ssh", "-T", "-F", connection["config_file"],
                "-i", connection["identity_file"], "-p", str(connection["port"]),
                "-o", f'UserKnownHostsFile="{connection["known_hosts_file"]}"',
                "-o", "StrictHostKeyChecking=yes", "--",
                f'{connection["user"]}@127.0.0.1', "printf rumux-ssh-fixture"], capture_output=True, text=True, timeout=10)
            assert result.returncode == 0 and result.stdout == "rumux-ssh-fixture", (result.stdout,result.stderr)
            print(json.dumps({"ssh_fixture":"passed"}))
