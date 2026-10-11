# Compatibility

This document defines the current compatibility and support expectations for `rumux`.

## Platform Matrix

| Surface | Linux x86_64 | macOS Apple Silicon | Windows x86_64 |
|---------|---------------|---------------------|----------------|
| CLI release binaries | Supported | Supported | Supported via release zip |
| CLI source build | Supported | Supported | Supported |
| Desktop source build and native checks | Validated | Validated | Validated |
| Installed desktop package | Validated `tar.gz` and user installer | Validated `.app` zip and user installer | Validated zip and PowerShell user installer |

Notes:

- Linux desktop builds require the GPUI display libraries plus GTK 3 and WebKitGTK 4.1. Browser panes require an X11/XWayland parent; Rumux selects it when `DISPLAY` is available. `RUMUX_LINUX_BACKEND=wayland` retains the native Wayland terminal shell with an explicit browser-unavailable message.
- macOS desktop packages require macOS 14 or newer and are unsigned and not notarized.
- Windows packages are unsigned and require DirectX 11, WebView2 Runtime and
  Microsoft's latest [Visual C++ v14 x64 runtime](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/).
  Installers include
  both desktop and CLI, verify extracted file hashes, and leave config/history
  outside the installation. The published `v0.1.0` release has CLI assets only;
  desktop packages await a future qualified release.
- The completion branch adds native build/test/smoke gates for Linux, macOS and
  Windows. All eight gates passed at `32b0ad3` in
  [CI run 38180275930](https://github.com/LukasGuptaLeary/rumux/actions/runs/38180275930),
  including debug layout, installed packages, native SSH and private state.
  Package manifests identify the same clean PR merge commit
  `0084b39599e17662780ede3dec702151d772ccbf`.
- Native package gates extract and install into owned directories with spaces,
  reject tampered files, validate metadata, launch the installed CLI/desktop and
  exercise its terminal service, browser and SSH. Windows CI uses the native
  Windows client against an isolated WSL1 POSIX target; users need an OpenSSH
  client and a remote POSIX shell, not a local WSL installation. These gates run
  independently of debug-build checks; both are required acceptance evidence.
- Native coverage is Linux X11/XWayland, macOS ARM64, Windows cmd.exe and remote
  POSIX shells. Other shell configurations and display/driver combinations
  are additional compatibility targets. Linux CI archives are built on Ubuntu
  24.04 and the qualified x86-64 binary requires glibc 2.39 or newer; build from
  source for older or otherwise incompatible distributions.

## On-Disk State

`rumux` currently persists desktop state under the platform config directory used by the app.

Compatibility expectations:

- Session files are versioned with `schema_version`.
- Additive changes to persisted session/config data should remain backward compatible within the current release line whenever practical.
- If a breaking persisted-state change is required, `rumux` should either:
  - migrate the data automatically, or
  - discard only the incompatible portion and document the behavior in release notes.
- Forward compatibility is not guaranteed. Older builds may refuse to load state created by newer builds.
- Saves replace the session atomically and retain `session.backup.json` from the
  preceding valid save. Recovery preserves the damaged primary as an
  `invalid-<uuid>.json` file. If no valid backup exists, or the primary has a
  newer schema, the app reports the failure and disables session writes for that
  run so the original can be recovered.
- A per-profile terminal service keeps PTYs running when the desktop exits.
  Stable IDs reattach to the same processes. Explicit pane/workspace close
  terminates those processes and removes stored history. `rumux session stop`
  stops all processes in the selected profile and retains history.
- Version 1 atomic history files retain bounded primary-screen cells, colors
  and launch-directory metadata. A cold service restart starts fresh shells and
  restores history; it does not resurrect dead processes. Changed history is
  checkpointed every five seconds and at normal service shutdown.
- Unsupported/corrupt history and incompatible live service protocols are
  preserved and reported. `persist_scrollback = false` disables disk history
  for newly started panes. Private Unix modes and Windows ACLs protect the
  service directory and endpoint; Windows runtime qualification is pending.

## Worktree Layout Compatibility

The repository/worktree contract is intentionally stable:

- `rumux` uses the same `.worktrees/` layout as `cmux`.
- Existing `.cmux/setup` hooks continue to work when `.rumux/setup` is absent.
- Branch-name sanitization and worktree directory expectations should not change without an explicit migration note.

## RPC Compatibility

The local desktop RPC contract is available for editor and agent integration, but it is still considered an evolving API during the current `0.1.x` line.

Expectations:

- Documented and advertised methods should keep their core behavior stable within a patch release.
- New fields may be added to JSON responses without being considered a breaking change.
- Method removal or semantic changes should be called out in the changelog and release notes.
- The `rumux mcp` bridge is a compatibility layer over this same local RPC contract, not a separate control plane.
- Workspace and surface summary `id` fields are stable UUIDs, persisted in
  schema 3 and later. Schema 4 adds browser panes and URLs; schema 5 adds typed
  SSH connections and remote launch directories. Older sessions receive IDs on load. Selectors accept `workspace_id`
  and `surface_id`; indices are retained for compatibility. Copies get fresh
  IDs. Long-running requests pin their initial target.
- Direct TCP clients now require the profile's `auth_token` on every request.
  The CLI/MCP bridge handle authentication automatically. This intentionally
  closes the previous unauthenticated loopback control path.

SSH workspaces require an OpenSSH client and a remote shell accepting POSIX
command syntax. SSH clients, remote shell PIDs and forwarded listeners survive
desktop restart while the terminal service remains running. A cold service
restart creates fresh connections. Linux/macOS loopback SSH tests prove both
lifecycles using temporary keys with strict known-host verification; Windows
SSH runtime remains unqualified.

## Desktop Instance Model

- `rumux-app` is intended to run as a single desktop instance per user-scoped IPC endpoint.
- A second launch should activate the already-running instance and exit instead of opening a parallel shell window.
- Alternate endpoints configured with `RUMUX_SOCKET_ADDR`, `RUMUX_SOCKET_PATH`, or `RUMUX_RUNTIME_DIR` are treated as separate instances for development and testing.
- Terminal ownership is scoped by `RUMUX_CONFIG_DIR`; isolate it as well when
  testing separate profiles. Multiple simultaneous GUI endpoints sharing a
  config profile are not qualified.

## Release and Support Policy

- `main` is the active development branch and may change without notice.
- Tagged releases are the supported installation points for external users.
- The current target cadence is small, incremental releases rather than long-lived stabilization branches.
- There is no LTS branch yet.
- Security or severe regression fixes should land in the next available release rather than waiting for a large batch.
