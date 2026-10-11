#!/usr/bin/env python3
"""Exercise a real desktop app/PTY in an isolated profile (requires a display).

Usage: python3 scripts/smoke-desktop.py --app target/debug/rumux-app --cli target/debug/rumux
No user session, config, repository files, or clipboard contents are modified.
"""
import argparse
import concurrent.futures
import json
import http.server
import os
from pathlib import Path
import shlex
import socket
import subprocess
import sys
import tempfile
import threading
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
    parser.add_argument("--tcp", action="store_true", help="Exercise authenticated loopback TCP on Unix too")
    parser.add_argument("--browser", action="store_true", help="Exercise the embedded browser against local fixture pages")
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
        env.pop("RUMUX_SURFACE_ID", None)
        windows = os.name == "nt"
        tcp = windows or args.tcp
        env["SHELL"] = "cmd.exe" if windows else "/bin/sh"
        env["RUMUX_SHELL"] = env["SHELL"]
        if tcp:
            with socket.socket() as probe:
                probe.bind(("127.0.0.1", 0))
                address = probe.getsockname()
            env["RUMUX_SOCKET_ADDR"] = f"{address[0]}:{address[1]}"
        else:
            endpoint = str(runtime / "rumux.sock")
            env["RUMUX_SOCKET_PATH"] = endpoint

        def rpc(method, params=None):
            stream = socket.socket(socket.AF_INET if tcp else socket.AF_UNIX)
            stream.settimeout(15)
            with stream:
                stream.connect(address if tcp else endpoint)
                request = {"id": "smoke", "method": method, "params": params or {}}
                if tcp:
                    request["auth_token"] = (config / f"rpc-{address[1]}.token").read_text(encoding="utf-8").strip()
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
            if windows:
                code = ("import ctypes; k=ctypes.windll.kernel32; "
                        "k.GetStdHandle.restype=ctypes.c_void_p; "
                        "k.GetConsoleMode.argtypes=[ctypes.c_void_p,ctypes.POINTER(ctypes.c_ulong)]; "
                        "k.SetConsoleMode.argtypes=[ctypes.c_void_p,ctypes.c_ulong]; "
                        "h=k.GetStdHandle(-11); mode=ctypes.c_ulong(); "
                        "assert k.GetConsoleMode(h,ctypes.byref(mode)); "
                        "assert k.SetConsoleMode(h,mode.value|4); " + code)
            argv = [sys.executable, "-c", code]
            return subprocess.list2cmdline(argv) if windows else shlex.join(argv)

        try:
            launch()
            checks.append("startup and RPC")
            if tcp:
                with socket.create_connection(address, timeout=5) as unauthorized:
                    unauthorized.sendall(b'{"id":"unauthorized","method":"system.ping","params":{}}\n')
                    with unauthorized.makefile("rb") as reader:
                        response = json.loads(reader.readline())
                    assert not response["ok"] and "authentication" in response["error"]
                checks.append("unauthenticated TCP client rejected")
            second = subprocess.run([str(app)], env=env, cwd=root, capture_output=True, timeout=10)
            assert second.returncode == 0, second.stderr.decode(errors="replace")
            checks.append("single-instance handoff")
            methods = rpc("system.capabilities")["methods"]
            assert "notification.focus" in methods and "surface.run_command" in methods

            workspace = rpc("workspace.create", {"name": "smoke-work"})
            work, workspace_id = workspace["index"], workspace["id"]
            surface = rpc("surface.split", {"placement": "right"})["created"]
            source, surface_id = surface["index"], surface["id"]
            target = {"workspace_index": work, "surface_index": source}
            command = python_command("print('smoke-command-result')")
            result = rpc("surface.run_command", dict(target, command=command, timeout_ms=5000))
            assert result["completed"] and result["exit_code"] == 0 and "smoke-command-result" in result["output"], result
            checks.append("real PTY command and output")
            rpc("surface.set_status", {"surface_id": surface_id, "status": "blocked", "message": "review requested"})
            reported = next(s for s in rpc("surface.list")["surfaces"] if s["id"] == surface_id)
            assert reported["status"] == "blocked" and reported["status_message"] == "review requested"
            marker = python_command("import sys;sys.stdout.write('\\x1b]133;A\\x07');sys.stdout.flush()")
            rpc("surface.send_text", dict(target, text=marker, append_newline=True))
            eventually(lambda: next(s for s in rpc("surface.list")["surfaces"] if s["id"] == surface_id)["status"] == "idle")
            checks.append("agent status and shell prompt activity")

            rpc("workspace.select", {"index": 0})
            notify = python_command("import sys,time;sys.stdout.write('\\x1b]777;notify;Chunked;review');sys.stdout.flush();time.sleep(.05);sys.stdout.write('\\x1b\\\\');sys.stdout.flush()")
            rpc("surface.send_text", dict(target, text=notify, append_newline=True))
            notification = eventually(lambda: next((n for n in rpc("notification.list") if n["title"] == "Chunked"), None))
            assert notification["workspace_id"] == work and notification["surface_index"] == source
            assert rpc("workspace.current")["index"] == 0
            rpc("notification.focus", {"id": notification["id"]})
            assert rpc("workspace.current")["index"] == work
            eventually(lambda: next(s for s in rpc("surface.list")["surfaces"] if s["target"])["index"] == source)
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
            idle = socket.socket(socket.AF_INET if tcp else socket.AF_UNIX)
            idle.connect(address if tcp else endpoint)
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
            with concurrent.futures.ThreadPoolExecutor(max_workers=1) as executor:
                pending = executor.submit(rpc, "surface.wait_for_output", {"surface_id": surface_id, "pattern": "stable-id-marker", "timeout_ms": 4000})
                time.sleep(0.1)
                rpc("workspace.close", {"index": 0})
                rpc("surface.send_text", {"surface_id": surface_id, "text": "echo stable-id-marker", "append_newline": True})
                assert pending.result(timeout=6)["matched"]
            assert rpc("workspace.current")["id"] == workspace_id
            assert rpc("surface.list", {"workspace_id": workspace_id})["surfaces"]
            checks.append("stable IDs across workspace reindexing and asynchronous calls")
            note = next(n for n in rpc("notification.list") if n["id"] == note["id"])
            assert note["workspace_id"] == 0
            rpc("notification.focus", {"id": note["id"]})
            assert rpc("workspace.current")["name"] == "smoke-work"
            assert rpc("workspace.current")["id"] == workspace_id
            assert any(s["id"] == surface_id for s in rpc("surface.list")["surfaces"])
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
            original_ids = {s["id"] for s in rpc("surface.list")["surfaces"]}
            duplicate = rpc("workspace.duplicate", {"workspace_id": workspace_id})
            assert rpc("workspace.current")["id"] != workspace_id, duplicate
            assert original_ids.isdisjoint({s["id"] for s in rpc("surface.list")["surfaces"]})
            checks.append("duplicated workspaces receive independent identities")
            if args.browser:
                class Fixture(http.server.BaseHTTPRequestHandler):
                    def log_message(self, *args):
                        pass
                    def do_GET(self):
                        second = self.path == "/second"
                        content = ("<title>Rumux second page</title><h1>Second page</h1>" if second else
                                   "<title>Rumux browser smoke</title><h1>Browser fixture marker</h1>"
                                   "<input id='secret' type='password' value='private-password'><input id='value'><button id='apply' onclick=\"document.querySelector('#result').textContent=document.querySelector('#value').value\">Apply</button>"
                                   "<p id='result'></p><a href='/second'>Second page</a>")
                        body = ("<!doctype html><html><body>" + content + "</body></html>").encode()
                        self.send_response(200)
                        self.send_header("Content-Type", "text/html; charset=utf-8")
                        self.send_header("Content-Length", str(len(body)))
                        self.end_headers()
                        self.wfile.write(body)
                with http.server.ThreadingHTTPServer(("127.0.0.1", 0), Fixture) as fixture:
                    thread = threading.Thread(target=fixture.serve_forever, daemon=True)
                    thread.start()
                    url = f"http://127.0.0.1:{fixture.server_port}/"
                    try:
                        browser = rpc("browser.create", {"url": url, "placement": "right"})["created"]
                        browser_target = {"surface_id": browser["id"]}
                        assert browser["kind"] == "browser"
                        eventually(lambda: rpc("browser.snapshot", browser_target)["value"]["title"] == "Rumux browser smoke")
                        assert rpc("browser.eval", dict(browser_target, expression="1 + 2"))["value"] == 3
                        snapshot = rpc("browser.snapshot", browser_target)["value"]
                        assert "Browser fixture marker" in snapshot["text"] and any(e["id"] == "apply" for e in snapshot["elements"])
                        filled = "Quoted \"text\" with $() and Unicode café"
                        rpc("browser.fill", dict(browser_target, selector="#value", text=filled))
                        rpc("browser.click", dict(browser_target, selector="#apply"))
                        assert rpc("browser.eval", dict(browser_target, expression="document.querySelector('#result').textContent"))["value"] == filled
                        secret = next(e for e in snapshot["elements"] if e["id"] == "secret")
                        assert "value" not in secret
                        for method, parameters in [
                            ("browser.navigate", dict(browser_target, url="file:///etc/passwd")),
                            ("browser.eval", dict(browser_target, expression="throw new Error('expected failure')")),
                            ("browser.eval", dict(browser_target, expression="Promise.resolve(3)")),
                            ("browser.click", dict(browser_target, selector="#missing")),
                        ]:
                            try:
                                rpc(method, parameters)
                            except ValueError:
                                pass
                            else:
                                raise AssertionError(f"{method} should reject {parameters}")
                        assert rpc("system.ping")["pong"]
                        checks.append("native browser DOM, evaluation, fill and click")
                        checks.append("browser error isolation and password snapshot privacy")

                        browser_requests = [
                            {"jsonrpc":"2.0","id":1,"method":"tools/list"},
                            {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"snapshot_browser","arguments":browser_target}},
                        ]
                        browser_mcp = subprocess.run([str(cli), "mcp"], input="".join(json.dumps(r) + "\n" for r in browser_requests), env=env, capture_output=True, text=True, timeout=10)
                        replies = [json.loads(line) for line in browser_mcp.stdout.splitlines()]
                        assert browser_mcp.returncode == 0 and len(replies) == 2, browser_mcp.stdout
                        tools = {tool["name"]:tool for tool in replies[0]["result"]["tools"]}
                        assert "surface_id" in tools["snapshot_browser"]["inputSchema"]["properties"]
                        assert replies[1]["result"]["structuredContent"]["value"]["title"] == "Rumux browser smoke"
                        checks.append("MCP native browser control")

                        rpc("surface.rename", dict(browser_target, name="Preview tab"))
                        terminal = rpc("surface.split", dict(browser_target, placement="down"))["created"]
                        assert terminal["kind"] == "terminal" and terminal["id"] != browser["id"]
                        rpc("surface.close", {"surface_id":terminal["id"]})
                        tab = rpc("surface.create", browser_target)["created"]
                        eventually(lambda: not rpc("browser.state", browser_target)["visible"])
                        rpc("surface.prev", {"surface_id":tab["id"]})
                        eventually(lambda: rpc("browser.state", browser_target)["visible"])
                        rpc("surface.next", browser_target)
                        eventually(lambda: not rpc("browser.state", browser_target)["visible"])
                        rpc("surface.close", {"surface_id":tab["id"]})
                        rpc("surface.focus", browser_target)
                        rpc("surface.zoom", browser_target)
                        assert rpc("browser.state", browser_target)["visible"]
                        rpc("surface.zoom", browser_target)
                        other = rpc("workspace.create", {"name":"browser visibility"})
                        eventually(lambda: not rpc("browser.state", browser_target)["visible"])
                        rpc("workspace.close", {"index":other["index"]})
                        eventually(lambda: rpc("browser.state", browser_target)["visible"])
                        checks.append("mixed browser/terminal splits, tabs, zoom and workspace visibility")
                        rpc("browser.navigate", dict(browser_target, url=url + "second"))
                        eventually(lambda: rpc("browser.snapshot", browser_target)["value"]["title"] == "Rumux second page")
                        rpc("browser.back", browser_target)
                        eventually(lambda: rpc("browser.snapshot", browser_target)["value"]["title"] == "Rumux browser smoke")
                        rpc("browser.reload", browser_target)
                        eventually(lambda: rpc("browser.snapshot", browser_target)["value"]["title"] == "Rumux browser smoke")
                        checks.append("browser navigation, history and reload")
                        stop()
                        launch()
                        restored = next(s for s in rpc("surface.list")["surfaces"] if s["id"] == browser["id"])
                        assert restored["kind"] == "browser" and restored["url"] == url
                        eventually(lambda: rpc("browser.snapshot", browser_target)["value"]["title"] == "Rumux browser smoke")
                        rpc("surface.close", browser_target)
                        assert all(s["id"] != browser["id"] for s in rpc("surface.list")["surfaces"])
                        checks.append("browser identity, URL restart restore and close")
                    finally:
                        fixture.shutdown()
                        thread.join(timeout=5)
        except BaseException:
            try:
                print(json.dumps({"surfaces":rpc("surface.list"),"last_terminal":rpc("surface.read",{"scope":"buffer","max_chars":4000})}, indent=2), file=sys.stderr)
            except (OSError,ValueError):
                pass
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
