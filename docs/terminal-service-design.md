# Durable terminal service

This implements the open session-lifecycle acceptance boundary in ROADMAP.md.
The service follows herdr's separate PTY ownership model while retaining Rumux's
GPUI shell. Native macOS and Linux suites prove live detach and cold history
restore. Windows runtime qualification is pending.

The desktop starts a per-profile headless service from the installed app binary.
It owns PTY masters, child waiters and the authoritative Alacritty parser. Closing
the desktop detaches its views. Closing a pane or workspace explicitly closes
its terminals and deletes their disk history. `rumux session status [--json]`
lists owned processes; `rumux session stop` terminates the selected service and
retains its history. The service does not exit merely because the desktop is
absent.
The service has a private endpoint descriptor, process lock and random token;
connections use authenticated loopback with bounded requests, responses and
worker counts. An incompatible protocol reports an error and preserves the
running service instead of silently replacing it.

Stable pane UUIDs select existing processes on reattach. A new pane supplies its
validated launch directory or typed SSH connection. Tabs and workspace copies
have distinct UUIDs and processes. The service retains title, remote/local CWD,
agent status and a bounded notification queue while the desktop is absent.
Terminal queries are answered by the service, so a detached shell is not blocked
waiting for a UI response.

Views receive parsed render state, not a truncated raw output log. Initial
attach supplies the grid and history; subsequent long polls supply the current
screen, cursor, mode/colors and history appended after a monotonic history
cursor. Resize, alternate-screen changes or an obsolete cursor request a full
snapshot. Cells use run-length encoding and every packet is validated before
allocation/indexing. Bounds are 64 retained panes, 512 columns, 256 rows, 10,000
history rows and one million cells per pane. Requests are capped at 1 MiB,
compressed replies at 16 MiB and decoded replies at 192 MiB. Worker count is
bounded to 96; long polls wait at most one second. Input queues are bounded;
resize requests are coalesced and carry actual cell pixel metrics.
The client keeps local selection, search and viewport state. Its mirror does
not feed PTY bytes back into a VTE parser. This preserves fragmented escapes,
UTF-8 and alternate-screen state without replaying stale terminal side effects.

Cold service restart restores bounded prior primary-screen history to a fresh
shell; it does not claim to resurrect a dead process. Disk history uses private,
versioned, atomic files, checkpointed every five seconds when changed and at
normal shutdown. `persist_scrollback = false` opts newly started panes out of
disk history. Existing processes retain their initial setting. Unix mode
700/600 and Windows owner ACLs protect the service directory and files.
Future versions and corrupt files are rejected and preserved. Live
detach/reattach and cold history restore are
separate tests. Smoke tests own an isolated service and stop it during cleanup.

The native suite verifies the same shell PID and a progressing background task
across desktop exit/restart, input to the resumed shell, scrollback and colors,
alternate-screen/fragmented-output reattach, resize, explicit close/reaping,
authenticated IPC, slow-client isolation, SSH forwarding across detach, cold
history restore, future/corrupt history preservation and disabled persistence.
macOS/Linux checks have executed; native Windows checks remain required.

The authoritative parser remains in the service, including fragmented VTE/OSC
input, primary and alternate grids and terminal query responses. Notifications
are acknowledged after delivery to the desktop. Terminal process exit and IPC
disconnect are distinct events. View teardown only detaches; docking layout
removal explicitly releases the service-owned UUID off the foreground thread.

The published Alacritty 0.25.1 crate is pinned locally with three narrow APIs:
a monotonic history cursor, read-only inactive primary grid access and a method
to install render mode/colors/cursor style. Its original source, license and
reference fixtures are retained. See its `RUMUX-PATCHES.md` for provenance and
removal conditions. No herdr source was copied.
