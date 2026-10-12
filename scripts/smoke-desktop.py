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
import re
from pathlib import Path
import shlex
import shutil
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
    parser.add_argument("--ssh", action="store_true", help="Exercise SSH terminals and forwarding against a local ephemeral sshd")
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
        env["RUMUX_TEST_APP"] = str(app)
        env["RUMUX_TERMINAL_TRACE"] = "1"
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
        debugger = None
        debugger_log = None
        debugger_finished = False
        debugger_path = root / "debugger.log"

        def finish_debugger():
            nonlocal debugger_finished
            if debugger is None or debugger_finished:
                return
            try:
                debugger.wait(timeout=10)
            except subprocess.TimeoutExpired:
                debugger.terminate()
                debugger.wait(timeout=5)
            debugger_log.flush()
            trace = debugger_path.read_text(encoding="utf-8", errors="replace")
            print(trace, file=sys.stderr)
            from windows_stack import symbolize
            try:
                print(symbolize(app, trace), file=sys.stderr)
            except (OSError, ValueError, AssertionError, subprocess.SubprocessError) as error:
                print(f"Native stack symbolization failed: {error}", file=sys.stderr)
            debugger_finished = True

        def trace_mixed_layout_close():
            nonlocal debugger, debugger_log
            if not windows or not os.environ.get("RUMUX_TRACE_WINDOWS_CLOSE"):
                return
            candidates = [r"C:\msys64\mingw64\bin\gdb.exe",
                          r"C:\msys64\ucrt64\bin\gdb.exe", shutil.which("gdb")]
            gdb = next((path for path in candidates if path and Path(path).is_file()), None)
            assert gdb, "Native Windows close diagnostics require the runner's GDB"
            debugger_log = debugger_path.open("w", encoding="utf-8")
            # Some Windows GDB distributions split multiword -ex arguments
            # differently from Python's CommandLineToArgvW quoting. A command
            # file keeps every debugger instruction intact and avoids MSYS paths.
            commands = root / "debugger-commands.gdb"
            commands.write_text("\n".join([
                "set pagination off", "set confirm off", "set print frame-arguments none",
                "set print thread-events off", 'file "' + app.as_posix() + '"',
                f"attach {process.pid}", "echo RUMUX_DEBUGGER_ATTACHED\\n", "continue",
                "info sharedlibrary", "info files", "thread apply all bt 120", "detach", ""
            ]), encoding="utf-8")
            debugger = subprocess.Popen([gdb, "--batch", "--nx", "-q", "--command=" + commands.as_posix()],
                stdout=debugger_log, stderr=debugger_log)
            eventually(lambda: "RUMUX_DEBUGGER_ATTACHED" in debugger_path.read_text(encoding="utf-8", errors="replace"))

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
                # cmd.exe consumes physical newlines as Enter even inside a
                # pasted command. Keep multiline fixture programs in one argument.
                code = "exec(" + repr(code) + ")"
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
                        trace_mixed_layout_close()
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
            from terminal_service_checks import exercise_service
            exercise_service(rpc, launch, stop, python_command, cli, env, root,
                             config, windows, checks, eventually)
            if args.ssh:
                from ssh_fixture import SshFixture
                if windows:
                    from windows_ssh_fixture import WslSshFixture as SshFixture
                with SshFixture(root) as fixture:
                    connection = fixture.connection
                    create = subprocess.run([str(cli), "ssh", f'{connection["user"]}@{connection["host"]}',
                        "--port", str(connection["port"]), "--cwd", connection["directory"],
                        "--identity-file", connection["identity_file"], "--known-hosts-file", connection["known_hosts_file"],
                        "--config-file", connection["config_file"], "--forward", f'{fixture.local_port}:{fixture.server.server_port}',
                        "--name", "SSH smoke", "--json"], env=env, capture_output=True, text=True, timeout=10)
                    assert create.returncode == 0, create.stderr
                    remote_workspace = json.loads(create.stdout)["result"]
                    remote_id = remote_workspace["id"]
                    remote = next(s for s in rpc("surface.list")["surfaces"] if s.get("ssh"))
                    remote_target = {"surface_id":remote["id"]}
                    result = rpc("surface.run_command", dict(remote_target, command="printf '%s\\n' SSH_CONNECTED; pwd; printf '%s\\n' \"$TERM_PROGRAM\" \"$RUMUX_SURFACE_ID\"", timeout_ms=10000))
                    assert result["completed"] and result["exit_code"] == 0, result
                    assert str(fixture.directory) in result["output"] and remote["id"] in result["output"] and "rumux" in result["output"], result
                    checks.append("SSH PTY, quoted launch directory and remote surface environment")

                    mcp_connection = dict(connection, forwards=[])
                    requests = [
                        {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}},
                        {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"create_ssh_workspace","arguments":{"name":"MCP SSH", "ssh":mcp_connection}}},
                    ]
                    mcp = subprocess.run([str(cli), "mcp"], input="".join(json.dumps(r)+"\n" for r in requests),
                                         text=True, capture_output=True, env=env, timeout=10)
                    responses = [json.loads(line) for line in mcp.stdout.splitlines()]
                    assert mcp.returncode == 0 and not responses[1]["result"]["isError"], (mcp.stderr, responses)
                    mcp_workspace = responses[1]["result"]["structuredContent"]
                    mcp_surface = next(s for s in rpc("surface.list")["surfaces"] if s.get("ssh"))
                    result = rpc("surface.run_command", {"surface_id":mcp_surface["id"],"command":"printf MCP_SSH_CONNECTED", "timeout_ms":10000})
                    assert result["completed"] and "MCP_SSH_CONNECTED" in result["output"], result
                    rpc("workspace.close", {"workspace_id":mcp_workspace["id"]})
                    rpc("workspace.select", {"workspace_id":remote_id})
                    checks.append("typed SSH CLI and MCP to real remote PTYs")

                    change = "cd -- " + shlex.quote(str(fixture.changed_directory)) + "; " + fixture.python_command(
                        f"import sys;sys.stdout.write('\\x1b]7;{fixture.changed_directory.as_uri()}\\x07');sys.stdout.flush()")
                    rpc("surface.send_text", dict(remote_target, text=change, append_newline=True))
                    eventually(lambda: next(s for s in rpc("surface.list")["surfaces"] if s["id"] == remote["id"])["cwd"] == str(fixture.changed_directory))
                    inherited = rpc("surface.split", dict(remote_target, placement="down"))["created"]
                    assert inherited["ssh"]["directory"] == str(fixture.changed_directory) and not inherited["ssh"]["forwards"], inherited
                    result = rpc("surface.run_command", {"surface_id":inherited["id"],"command":"pwd","timeout_ms":10000})
                    assert result["completed"] and str(fixture.changed_directory) in result["output"], result
                    checks.append("remote directory inheritance and independent SSH splits")
                    erased = rpc("surface.run_command", {"surface_id": inherited["id"],
                        "command": "printf '\\033[2J\\033[3J\\033[HREMOTE_ERASED_OUTPUT\\n'; exit 7", "timeout_ms": 10000})
                    assert erased["completed"] and erased["exit_code"] == 7 and erased["truncated"], erased
                    assert "REMOTE_ERASED_OUTPUT" in erased["output"], erased
                    checks.append("remote command completion survives screen/history erasure")

                    preview = None
                    if args.browser:
                        preview = rpc("browser.create", dict(remote_target, url=f"http://127.0.0.1:{fixture.local_port}/", placement="right"))["created"]
                        eventually(lambda: rpc("browser.snapshot", {"surface_id":preview["id"]})["value"]["title"] == "SSH forwarded preview")
                        checks.append("browser preview through loopback SSH forwarding")

                    copy = rpc("workspace.duplicate", {"workspace_id":remote_id})
                    copied_surfaces = rpc("surface.list")["surfaces"]
                    assert all(not s["ssh"]["forwards"] for s in copied_surfaces if s.get("ssh"))
                    assert all(s["id"] not in [remote["id"],inherited["id"]] for s in copied_surfaces)
                    rpc("workspace.close", {"index":copy["index"]})
                    rpc("workspace.select", {"workspace_id":remote_id})
                    from terminal_service_checks import service_rpc
                    client_pid=next(p["pid"] for p in service_rpc(config,"list")["result"] if p["id"]==remote["id"])
                    rpc("surface.send_text",dict(remote_target,text="printf 'REMOTE_SHELL_BEFORE_%s\\n' \"$$\"",append_newline=True))
                    def remote_shell_pid(marker):
                        text=rpc("surface.read",dict(remote_target,scope="buffer",max_chars=100000))["text"]
                        matches=re.findall(marker+r"_(\d+)",text)
                        return matches[-1] if matches else None
                    remote_pid=eventually(lambda:remote_shell_pid("REMOTE_SHELL_BEFORE"))
                    stop()
                    assert next(p["pid"] for p in service_rpc(config,"list")["result"] if p["id"]==remote["id"])==client_pid
                    with socket.create_connection(("127.0.0.1",fixture.local_port),timeout=2):
                        pass
                    launch()
                    rpc("workspace.select", {"workspace_id":remote_id})
                    restored = next(s for s in rpc("surface.list")["surfaces"] if s["id"] == remote["id"])
                    assert restored["ssh"]["directory"] == str(fixture.changed_directory)
                    result = rpc("surface.run_command", dict(remote_target, command="pwd", timeout_ms=10000))
                    assert result["completed"] and str(fixture.changed_directory) in result["output"], result
                    rpc("surface.send_text",dict(remote_target,text="printf 'REMOTE_SHELL_AFTER_%s\\n' \"$$\"",append_newline=True))
                    assert eventually(lambda:remote_shell_pid("REMOTE_SHELL_AFTER"))==remote_pid
                    checks.append("same SSH client, remote shell and forwarded listener across desktop restart")
                    rpc("workspace.close", {"workspace_id":remote_id})
                    def forwarding_closed():
                        try:
                            with socket.create_connection(("127.0.0.1",fixture.local_port),timeout=.2):
                                return False
                        except OSError:
                            return True
                    eventually(forwarding_closed)
                    checks.append("SSH pane ownership closes its forwarded listener")
            completion_workspace = rpc("workspace.create", {"name": "erased command completion"})
            pane = rpc("surface.list")["surfaces"][0]
            # Native cls clears the viewport, whereas POSIX CSI 3J also erases
            # scrollback. Inspect the receipt independently on both platforms.
            erase = "cls & echo ERASED_OUTPUT & cmd.exe /c exit 7" if windows else "printf '\\033[2J\\033[3J\\033[HERASED_OUTPUT\\n'; exit 7"
            erased = rpc("surface.run_command", {"surface_id": pane["id"], "command": erase, "timeout_ms": 10000})
            assert erased["completed"] and erased["exit_code"] == 7, erased
            assert "ERASED_OUTPUT" in erased["output"], erased
            if not windows:
                assert erased["truncated"], erased
            from terminal_service_checks import service_rpc
            def receipts():
                return service_rpc(config, "poll", {"id": pane["id"]})["result"]["metadata"]["completed_commands"]
            receipt = eventually(receipts)
            assert len(receipt) == 1 and receipt[0][1] == 7, receipt
            assert re.fullmatch(r"[0-9a-f]{32}", receipt[0][0]), receipt
            flood = "import sys;sys.stdout.write(''.join('\\x1b]777;rumux-command;%032x;0\\x07'%i for i in range(20)));sys.stdout.flush();print('BOUNDED_RECEIPTS')"
            bounded = rpc("surface.run_command", {"surface_id": pane["id"], "command": python_command(flood), "timeout_ms": 10000})
            assert bounded["completed"] and bounded["exit_code"] == 0, bounded
            retained = eventually(lambda: receipts() if len(receipts()) == 16 else None)
            assert "0" * 32 not in [token for token, _ in retained], retained
            assert "%032x" % 19 in [token for token, _ in retained], retained
            rpc("workspace.close", {"workspace_id": completion_workspace["id"]})
            checks.append("local command completion receipt after clear")
            checks.append("bounded completion receipts in the authoritative service")
        except BaseException:
            finish_debugger()
            if windows and debugger is None and os.environ.get("RUMUX_TRACE_WINDOWS_CLOSE") and process is not None and process.poll() is None:
                # Capture startup hangs too, before any diagnostic RPC waits or
                # teardown. The normal layout debugger attaches later in the suite.
                candidates = [r"C:\msys64\mingw64\bin\gdb.exe", r"C:\msys64\ucrt64\bin\gdb.exe", shutil.which("gdb")]
                gdb = next((path for path in candidates if path and Path(path).is_file()), None)
                if gdb:
                    commands = root / "startup-failure.gdb"
                    commands.write_text("\n".join([
                        "set pagination off", "set confirm off", "set print frame-arguments none",
                        "set print thread-events off", 'file "' + app.as_posix() + '"',
                        f"attach {process.pid}", "info sharedlibrary", "info files",
                        "thread apply all bt 120", "detach", ""
                    ]), encoding="utf-8")
                    try:
                        result = subprocess.run([gdb, "--batch", "--nx", "-q", "--command=" + commands.as_posix()], capture_output=True, text=True, timeout=20)
                        trace = result.stdout + result.stderr
                        print(trace, file=sys.stderr)
                        from windows_stack import symbolize
                        print(symbolize(app, trace), file=sys.stderr)
                    except (OSError, ValueError, AssertionError, subprocess.SubprocessError) as error:
                        print(f"Startup stack diagnostics failed: {error}", file=sys.stderr)
            try:
                print(json.dumps({"surfaces":rpc("surface.list"),"last_terminal":rpc("surface.read",{"scope":"buffer","max_chars":4000})}, indent=2), file=sys.stderr)
                for surface in rpc("surface.list")["surfaces"]:
                    if surface["kind"] == "terminal":
                        print(json.dumps(rpc("surface.read", {"surface_id":surface["id"],"scope":"buffer","max_chars":4000}), indent=2),file=sys.stderr)
                        from terminal_service_checks import service_rpc, packet_text
                        snapshot = service_rpc(config, "poll", {"id": surface["id"]})["result"]
                        print(json.dumps({"daemon_surface_id":surface["id"],
                            "daemon_cursor":snapshot["packet"]["cursor"],
                            "daemon_text":packet_text(snapshot["packet"])[-4000:]}, indent=2),file=sys.stderr)
            except (OSError,ValueError):
                pass
            service_log = config / "terminals" / "service.log"
            if service_log.exists():
                print(service_log.read_text(encoding="utf-8"),file=sys.stderr)
            stop()
            log.flush()
            print(log_path.read_text(encoding="utf-8"), file=sys.stderr)
            raise
        finally:
            finish_debugger()
            if debugger_log is not None:
                debugger_log.close()
            stop()
            if (config / "terminal-service.json").exists():
                cleanup = subprocess.run([str(cli), "session", "stop"], env=env,
                                         capture_output=True, text=True, timeout=10)
                assert cleanup.returncode == 0, cleanup.stderr
                eventually(lambda: not (config / "terminal-service.json").exists())
            log.close()
    print(json.dumps({"passed": checks, "elapsed_seconds": round(time.monotonic() - started, 2)}, indent=2))


if __name__ == "__main__":
    main()
