#!/usr/bin/env python3
"""Exercise a real desktop app/PTY in an isolated profile (requires a display).

Usage: python3 scripts/smoke-desktop.py --app target/debug/rumux-app --cli target/debug/rumux
No user session, config, repository files, or clipboard contents are modified.
"""
import argparse
import concurrent.futures
import json
import os
from pathlib import Path
import shlex
import socket
import subprocess
import sys
import tempfile
import time


def eventually(predicate, timeout=10):
    deadline = time.monotonic() + timeout
    last_error = None
    while time.monotonic() < deadline:
        try:
            value = predicate()
            if value:
                return value
        except (OSError, ValueError, AssertionError) as error:
            last_error = error
        time.sleep(0.05)
    raise AssertionError(f"Condition did not become true: {last_error}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", default="target/debug/rumux-app")
    parser.add_argument("--cli", default="target/debug/rumux")
    args = parser.parse_args()
    app, cli = Path(args.app).resolve(), Path(args.cli).resolve()
    assert app.is_file() and cli.is_file(), "Build rumux-app and rumux-cli first"
    checks = []
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="rumux-smoke-") as directory:
        root = Path(directory)
        config, runtime = root / "config", root / "runtime"
        config.mkdir()
        runtime.mkdir()
        (config / "config.toml").write_text("desktop_notifications = false\n", encoding="utf-8")
        env = dict(os.environ, RUMUX_CONFIG_DIR=str(config), RUMUX_RUNTIME_DIR=str(runtime))
        env.pop("RUMUX_SOCKET_ADDR", None)
        env.pop("RUMUX_SOCKET_PATH", None)
        windows = os.name == "nt"
        env["SHELL"] = "cmd.exe" if windows else "/bin/sh"
        if windows:
            with socket.socket() as probe:
                probe.bind(("127.0.0.1", 0))
                address = probe.getsockname()
            env["RUMUX_SOCKET_ADDR"] = f"{address[0]}:{address[1]}"
        else:
            endpoint = str(runtime / "rumux.sock")
            env["RUMUX_SOCKET_PATH"] = endpoint

        def rpc(method, params=None):
            stream = socket.socket(socket.AF_INET if windows else socket.AF_UNIX)
            stream.settimeout(15)
            with stream:
                stream.connect(address if windows else endpoint)
                request = {"id": "smoke", "method": method, "params": params or {}}
                stream.sendall((json.dumps(request) + "\n").encode())
                with stream.makefile("rb") as reader:
                    response = json.loads(reader.readline(4 * 1024 * 1024))
            if not response.get("ok"):
                raise ValueError(response.get("error", "RPC failed"))
            return response["result"]

        log_path = root / "desktop.log"
        log = log_path.open("w+", encoding="utf-8")
        process = None

        def launch():
            nonlocal process
            process = subprocess.Popen([str(app)], env=env, cwd=root,
                                       stdin=subprocess.DEVNULL, stdout=log, stderr=log)
            eventually(lambda: process.poll() is None and rpc("system.ping")["pong"])

        def stop():
            nonlocal process
            if process is not None and process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
            process = None

        def python_command(code):
            argv = [sys.executable, "-c", code]
            return subprocess.list2cmdline(argv) if windows else shlex.join(argv)

        try:
            launch()
            checks.append("startup and RPC")
            second = subprocess.run([str(app)], env=env, cwd=root, capture_output=True, timeout=10)
            assert second.returncode == 0, second.stderr.decode(errors="replace")
            checks.append("single-instance handoff")
            methods = rpc("system.capabilities")["methods"]
            assert "notification.focus" in methods and "surface.run_command" in methods

            work = rpc("workspace.create", {"name": "smoke-work"})["index"]
            source = rpc("surface.split", {"placement": "right"})["created"]["index"]
            target = {"workspace_index": work, "surface_index": source}
            command = python_command("print('smoke-command-result')")
            result = rpc("surface.run_command", dict(target, command=command, timeout_ms=5000))
            assert result["completed"] and result["exit_code"] == 0 and "smoke-command-result" in result["output"], result
            checks.append("real PTY command and output")

            rpc("workspace.select", {"index": 0})
            notify = python_command("import sys,time;sys.stdout.write('\\x1b]777;notify;Chunked;review');sys.stdout.flush();time.sleep(.05);sys.stdout.write('\\x1b\\\\');sys.stdout.flush()")
            rpc("surface.send_text", dict(target, text=notify, append_newline=True))
            notification = eventually(lambda: next((n for n in rpc("notification.list") if n["title"] == "Chunked"), None))
            assert notification["workspace_id"] == work and notification["surface_index"] == source
            assert rpc("workspace.current")["index"] == 0
            rpc("notification.focus", {"id": notification["id"]})
            assert rpc("workspace.current")["index"] == work
            assert next(s for s in rpc("surface.list")["surfaces"] if s["target"])["index"] == source
            assert next(n for n in rpc("notification.list") if n["id"] == notification["id"])["read"]
            assert rpc("workspace.current")["unread_count"] == 0
            checks.append("fragmented hidden-pane notification and exact focus")

            new_cwd = root / "working directory"
            new_cwd.mkdir()
            directory_signal = python_command(f"import sys;sys.stdout.write('\\x1b]7;{new_cwd.as_uri()}\\x07');sys.stdout.flush()")
            rpc("surface.send_text", dict(target, text=directory_signal, append_newline=True))
            eventually(lambda: next(s for s in rpc("surface.list")["surfaces"] if s["index"] == source)["cwd"] == str(new_cwd))
            created = rpc("surface.create", target)["created"]
            assert created["cwd"] == str(new_cwd), created
            result = rpc("surface.run_command", {"surface_index": created["index"], "command": python_command("import os;print(os.getcwd())"), "timeout_ms": 5000})
            assert str(new_cwd) in result["output"], result
            checks.append("OSC 7 and new-terminal directory inheritance")

            with concurrent.futures.ThreadPoolExecutor(max_workers=1) as executor:
                pending = executor.submit(rpc, "surface.wait_for_output", dict(target, pattern="concurrent-client-marker", timeout_ms=4000))
                time.sleep(0.2)
                sent_at = time.monotonic()
                rpc("surface.send_text", dict(target, text="echo concurrent-client-marker", append_newline=True))
                assert time.monotonic() - sent_at < 2, "A waiting client blocked the sender"
                assert pending.result(timeout=6)["matched"]
            checks.append("concurrent output wait and input")

            # An idle client cannot prevent unrelated requests or remain forever.
            idle = socket.socket(socket.AF_INET if windows else socket.AF_UNIX)
            idle.connect(address if windows else endpoint)
            assert rpc("system.ping")["pong"]
            idle.close()
            failed = subprocess.run([str(cli), "rpc", "does.not.exist"], env=env, capture_output=True, timeout=10)
            assert failed.returncode != 0 and json.loads(failed.stdout)["ok"] is False
            checks.append("idle client isolation and CLI failure exit status")

            requests = [
                {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}},
                {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "rpc_raw", "arguments": {"method": "system.ping", "params": {}}}},
            ]
            mcp = subprocess.run([str(cli), "mcp"], input="".join(json.dumps(r) + "\n" for r in requests), env=env, capture_output=True, text=True, timeout=10)
            responses = [json.loads(line) for line in mcp.stdout.splitlines()]
            assert mcp.returncode == 0 and len(responses) == 2 and not responses[1]["result"].get("isError", False), mcp.stdout
            checks.append("MCP to desktop round trip")

            # Removing an earlier workspace must preserve notification ownership.
            note = rpc("notification.create", dict(target, title="ownership", body="test"))
            rpc("workspace.close", {"index": 0})
            note = next(n for n in rpc("notification.list") if n["id"] == note["id"])
            assert note["workspace_id"] == 0
            rpc("notification.focus", {"id": note["id"]})
            assert rpc("workspace.current")["name"] == "smoke-work"
            rpc("notification.clear")
            assert rpc("workspace.current")["unread_count"] == 0
            checks.append("workspace removal and notification ownership")

            rpc("surface.close", {"surface_index": created["index"]})
            count = len(rpc("surface.list")["surfaces"])
            assert count == 2
            eventually(lambda: (config / "session.json").is_file())
            session = json.loads((config / "session.json").read_text(encoding="utf-8"))
            assert session["workspaces"][0]["name"] == "smoke-work"
            stop()
            launch()
            assert rpc("workspace.current")["name"] == "smoke-work"
            assert len(rpc("surface.list")["surfaces"]) == count
            assert any(s["cwd"] == str(new_cwd) for s in rpc("surface.list")["surfaces"])
            checks.append("pane close and layout/directory restart restore")
        except BaseException:
            stop()
            log.flush()
            print(log_path.read_text(encoding="utf-8"), file=sys.stderr)
            raise
        finally:
            stop()
            log.close()
    print(json.dumps({"passed": checks, "elapsed_seconds": round(time.monotonic() - started, 2)}, indent=2))


if __name__ == "__main__":
    main()
