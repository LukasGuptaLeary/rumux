"""Owned-profile integration checks for persistent PTYs and parsed mirrors."""
import gzip
import json
import os
import socket
import struct
import subprocess
import time
import uuid


def service_rpc(config, method, params=None, *, token=None, protocol=1, error=False,
                fragmented=False):
    endpoint = json.loads((config / "terminal-service.json").read_text(encoding="utf-8"))
    host, port = endpoint["address"].rsplit(":", 1)
    request = {"protocol": protocol, "token": token or endpoint["token"],
               "method": method, "params": params or {}}
    data = json.dumps(request).encode()
    with socket.create_connection((host, int(port)), timeout=5) as stream:
        framed = struct.pack(">I", len(data)) + data
        if fragmented:
            # Accepted sockets can inherit O_NONBLOCK on macOS. Delayed header
            # and payload fragments must still be read completely by a worker.
            for fragment in (framed[:2], framed[2:12], framed[12:]):
                stream.sendall(fragment)
                time.sleep(.03)
        else:
            stream.sendall(framed)
        with stream.makefile("rb") as reader:
            header = reader.read(4)
            assert len(header) == 4, "Missing terminal service frame"
            length = struct.unpack(">I", header)[0]
            assert length <= 16 * 1024 * 1024
            encoded = reader.read(length)
            assert len(encoded) == length
            response = json.loads(gzip.decompress(encoded))
    if error:
        assert not response["ok"], response
    else:
        assert response["ok"], response.get("error")
    return response


def packet_text(packet):
    return "\n".join("".join(run["cell"]["c"] * run["count"] for run in row)
                     for row in packet["history"] + packet["screen"])


def assert_windows_private(path):
    # Read only ACL identities/rights; never emit file or token contents.
    script = ("$ErrorActionPreference='Stop'; "
              "$owner=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value; "
              "$acl=Get-Acl -LiteralPath $env:RUMUX_TEST_PRIVATE_PATH; "
              "$rules=$acl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier]); "
              "if (@($rules).Count -eq 0) { throw 'Missing owner access rule' }; "
              "foreach ($rule in $rules) { if ($rule.AccessControlType -eq 'Allow' "
              "-and $rule.IdentityReference.Value -ne $owner) { throw \"Unexpected allowed identity $($rule.IdentityReference.Value)\" } }")
    # Python inherits pwsh's module path in Actions. Windows PowerShell must
    # reconstruct its own path rather than attempting to load PowerShell 7 DLLs.
    env = {key: value for key, value in os.environ.items() if key.upper() != "PSMODULEPATH"}
    env["RUMUX_TEST_PRIVATE_PATH"] = str(path)
    result = subprocess.run(["powershell", "-NoProfile", "-Command", script],
        env=env, capture_output=True, text=True, timeout=10)
    assert result.returncode == 0, result.stderr


def exercise_service(rpc, launch, stop, python_command, cli, env, root, config,
                     windows, checks, eventually):
    workspace = rpc("workspace.create", {"name": "service lifecycle"})
    pane = rpc("surface.list")["surfaces"][0]
    target = {"surface_id": pane["id"]}
    service_target = {"id": pane["id"]}

    def snapshot():
        return service_rpc(config, "poll", service_target)["result"]

    def process_id():
        return next(p["pid"] for p in service_rpc(config, "list")["result"]
                    if p["id"] == pane["id"])

    shell_pid = process_id()
    service_pid = service_rpc(config, "ping")["result"]["pid"]
    status = subprocess.run([str(cli), "session", "status", "--json"], env=env,
                            capture_output=True, text=True, timeout=10)
    assert status.returncode == 0 and any(p["pid"] == shell_pid for p in json.loads(status.stdout))
    service_rpc(config, "ping", token="0" * 64, error=True)
    service_rpc(config, "ping", protocol=2, error=True)
    assert service_rpc(config, "ping", fragmented=True)["result"]["pid"] == service_pid
    endpoint = json.loads((config / "terminal-service.json").read_text(encoding="utf-8"))
    if windows:
        assert_windows_private(config / "terminal-service.json")
        assert_windows_private(config / "terminals")
    host, port = endpoint["address"].rsplit(":", 1)
    with socket.create_connection((host, int(port)), timeout=3) as idle:
        started = time.monotonic()
        assert service_rpc(config, "ping")["result"]["pid"] == service_pid
        assert time.monotonic() - started < 1
        # Advertise an oversized request without allocating its claimed payload.
        idle.sendall(struct.pack(">I", 1024 * 1024 + 1))
    checks.append("terminal service authentication, framing and idle-client isolation")

    # Prior color and enough rows to force real scrollback, independent of the viewport.
    code = "import sys;sys.stdout.write('\\x1b[38;2;17;34;51mPERSISTED_COLOUR\\x1b[0m\\r\\n' + ''.join('history_%03d\\r\\n'%i for i in range(90)));sys.stdout.flush()"
    result = rpc("surface.run_command", dict(target, command=python_command(code), timeout_ms=10000))
    assert result["completed"], result
    initial = snapshot()["packet"]
    assert "PERSISTED_COLOUR" in packet_text(initial)
    assert any(run["cell"]["fg"] == {"Spec": {"r": 17, "g": 34, "b": 51}}
               for row in initial["history"] for run in row)
    checks.append("parsed truecolor and scrollback snapshots")

    counter = root / "detached-counter"
    ticker = ("import pathlib,time,sys;p=pathlib.Path(" + repr(str(counter)) + ");"
              "\nfor i in range(1,21):\n p.write_text(str(i));print('OFFLINE_TICK_%d'%i,flush=True)\n"
              " if i==5: print('\\x1b]777;notify;Detached notice;service stayed alive\\x07',flush=True)\n time.sleep(.1)\n")
    launch_ticker = "import subprocess,sys;subprocess.Popen([sys.executable,'-u','-c'," + repr(ticker) + "]);print('TICKER_STARTED')"
    rpc("surface.send_text", dict(target, text=python_command(launch_ticker), append_newline=True))
    def counter_value():
        # write_text truncates before writing. A concurrent read can see an
        # empty file; return no observation until a complete number is visible.
        try:
            return int(counter.read_text())
        except (FileNotFoundError, ValueError):
            return 0

    before = eventually(counter_value)
    stop()
    eventually(lambda: counter_value() > before)
    assert process_id() == shell_pid
    assert service_rpc(config, "ping")["result"]["pid"] == service_pid
    assert "OFFLINE_TICK_" in packet_text(snapshot()["packet"])
    eventually(lambda: any(notice["title"] == "Detached notice" for notice in snapshot()["metadata"]["notifications"]))

    # An incompatible app refuses to replace the live owner or kill its processes.
    descriptor = config / "terminal-service.json"
    original = descriptor.read_bytes()
    incompatible = json.loads(original)
    incompatible["protocol"] = 999
    descriptor.write_text(json.dumps(incompatible), encoding="utf-8")
    try:
        probe = subprocess.run([str(env["RUMUX_TEST_APP"])], env=env, cwd=root,
                               capture_output=True, text=True, timeout=10)
        assert probe.returncode != 0 and "preserved" in probe.stderr, probe.stderr
        assert process_id() == shell_pid
    finally:
        descriptor.write_bytes(original)
    launch()
    assert process_id() == shell_pid
    eventually(lambda: "OFFLINE_TICK_" in rpc("surface.read", dict(target, scope="buffer", max_chars=100000))["text"])
    eventually(lambda: len([notice for notice in rpc("notification.list") if notice["title"] == "Detached notice"]) == 1)
    eventually(lambda: not snapshot()["metadata"]["notifications"])
    result = rpc("surface.run_command", dict(target, command="echo SERVICE_RESUMED", timeout_ms=10000))
    assert result["completed"] and "SERVICE_RESUMED" in result["output"], result
    eventually(lambda: counter_value() == 20)
    checks.append("same PTY process and progressing background job across desktop restart")
    checks.append("incompatible service preservation and resumed terminal input")

    # Leave the authoritative parser in a fragmented CSI while the UI detaches.
    half = root / "escape-half"
    continue_fragment = root / "escape-continue"
    finish_alternate = root / "escape-finish"
    fragment = ("import sys,time,pathlib;sys.stdout.write('\\x1b[?1049hALT_REATTACH\\x1b[38;2;17;34;');sys.stdout.flush();"
                "pathlib.Path(" + repr(str(half)) + ").write_text('ready')\n"
                "while not pathlib.Path(" + repr(str(continue_fragment)) + ").exists(): time.sleep(.02)\n"
                "sys.stdout.write('51mFRAGMENT_FINISHED');sys.stdout.flush()\n"
                "while not pathlib.Path(" + repr(str(finish_alternate)) + ").exists(): time.sleep(.02)\n"
                "sys.stdout.write('\\x1b[0m\\x1b[?1049l');sys.stdout.flush()")
    rpc("surface.send_text", dict(target, text=python_command(fragment), append_newline=True))
    eventually(lambda: half.exists())
    eventually(lambda: snapshot()["packet"]["cursor"]["mode"] & (1 << 12))
    stop()
    continue_fragment.write_text("continue", encoding="utf-8")
    eventually(lambda: "FRAGMENT_FINISHED" in packet_text(snapshot()["packet"]))
    alternate = snapshot()["packet"]
    assert alternate["cursor"]["mode"] & (1 << 12)
    assert any(run["cell"]["fg"] == {"Spec": {"r": 17, "g": 34, "b": 51}}
               for row in alternate["screen"] for run in row)
    launch()
    try:
        eventually(lambda: "FRAGMENT_FINISHED" in rpc("surface.read", dict(target, scope="visible"))["text"])
    except BaseException:
        import sys
        current = snapshot()["packet"]
        current["history"] = []
        print(json.dumps({"server_cursor":current["cursor"],"server_screen":packet_text(current),
                          "client_visible":rpc("surface.read",dict(target,scope="visible"))["text"],
                          "finish_file_exists":finish_alternate.exists()},indent=2),file=sys.stderr)
        raise
    finish_alternate.write_text("finish", encoding="utf-8")
    eventually(lambda: not snapshot()["packet"]["cursor"]["mode"] & (1 << 12))
    eventually(lambda: "OFFLINE_TICK_20" in rpc("surface.read", dict(target, scope="buffer", max_chars=100000))["text"])
    checks.append("alternate-screen and fragmented escape preservation across reattach")

    # Reject unsupported/corrupt disk state without replacing it or starting a PTY.
    rejected_id = str(uuid.uuid4())
    rejected_history = config / "terminals" / (rejected_id + ".history")
    future = {"ok": True, "result": {"version": 999, "packet": snapshot()["packet"],
              "metadata": snapshot()["metadata"]}, "error": None}
    encoded = gzip.compress(json.dumps(future).encode())
    try:
        for stored in (struct.pack(">I", len(encoded)) + encoded, b"corrupt history"):
            rejected_history.write_bytes(stored)
            failure = service_rpc(config, "open", {"id": rejected_id, "history": 1000}, error=True)
            assert "preserved" in failure["error"], failure
            assert rejected_history.read_bytes() == stored
            assert all(p["id"] != rejected_id for p in service_rpc(config, "list")["result"])
    finally:
        rejected_history.unlink(missing_ok=True)

    # Disabling persistence applies when a pane starts and also to the final
    # checkpoint at service shutdown, without needing a five-second sleep.
    disabled_id = str(uuid.uuid4())
    disabled_history = config / "terminals" / (disabled_id + ".history")
    settings = config / "config.toml"
    original_settings = settings.read_bytes()
    try:
        settings.write_bytes(original_settings + b"\npersist_scrollback = false\n")
        service_rpc(config, "open", {"id": disabled_id, "history": 1000})
    finally:
        settings.write_bytes(original_settings)
    service_rpc(config, "write", {"id": disabled_id, "bytes": list(b"echo PRIVATE_HISTORY\r")})
    eventually(lambda: "PRIVATE_HISTORY" in packet_text(service_rpc(config, "poll", {"id": disabled_id})["result"]["packet"]))

    stop()
    stopped = subprocess.run([str(cli), "session", "stop"], env=env,
                             capture_output=True, text=True, timeout=10)
    assert stopped.returncode == 0, stopped.stderr
    eventually(lambda: not descriptor.exists())
    assert not disabled_history.exists()
    checks.append("unsupported history preservation and disabled disk persistence")
    history = config / "terminals" / (pane["id"] + ".history")
    assert history.is_file()
    if not windows:
        assert history.stat().st_mode & 0o777 == 0o600
        assert history.parent.stat().st_mode & 0o777 == 0o700
    else:
        assert_windows_private(history)
    launch()
    assert process_id() != shell_pid
    assert service_rpc(config, "ping")["result"]["pid"] != service_pid
    eventually(lambda: "PERSISTED_COLOUR" in rpc("surface.read", dict(target, scope="buffer", max_chars=100000))["text"])
    restored = snapshot()["packet"]
    assert not restored["cursor"]["mode"] & (1 << 12)
    assert any(run["cell"]["fg"] == {"Spec": {"r": 17, "g": 34, "b": 51}}
               for row in restored["history"] for run in row)
    checks.append("atomic private cold-history restore to a fresh shell")
    rpc("workspace.close", {"workspace_id": workspace["id"]})
    eventually(lambda: all(p["id"] != pane["id"] for p in service_rpc(config, "list")["result"]))
    eventually(lambda: not history.exists())
    checks.append("explicit workspace close releases owned PTYs and stored history")
