# Compatibility

This document defines the current compatibility and support expectations for `rumux`.

## Platform Matrix

| Surface | Linux x86_64 | macOS Apple Silicon | Windows x86_64 |
|---------|---------------|---------------------|----------------|
| CLI release binaries | Supported | Not yet published through releases | Supported via release zip |
| CLI source build | Supported | Supported | Supported |
| Desktop source build | Supported | Supported | Not yet validated |
| Desktop release artifacts | Supported (`tar.gz`) | Supported (`.app` zip) | Not yet published |

Notes:

- Linux desktop builds assume the usual X11 and/or Wayland runtime libraries for GPUI.
- macOS desktop release artifacts are currently unsigned and not notarized.
- Windows desktop support is an active validation target, not a compatibility promise yet.

## On-Disk State

`rumux` currently persists desktop state under the platform config directory used by the app.

Compatibility expectations:

- Session files are versioned with `schema_version`.
- Additive changes to persisted session/config data should remain backward compatible within the current release line whenever practical.
- If a breaking persisted-state change is required, `rumux` should either:
  - migrate the data automatically, or
  - discard only the incompatible portion and document the behavior in release notes.
- Forward compatibility is not guaranteed. Older builds may refuse to load state created by newer builds.

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

## Desktop Instance Model

- `rumux-app` is intended to run as a single desktop instance per user-scoped IPC endpoint.
- A second launch should activate the already-running instance and exit instead of opening a parallel shell window.
- Alternate endpoints configured with `RUMUX_SOCKET_ADDR`, `RUMUX_SOCKET_PATH`, or `RUMUX_RUNTIME_DIR` are treated as separate instances for development and testing.

## Release and Support Policy

- `main` is the active development branch and may change without notice.
- Tagged releases are the supported installation points for external users.
- The current target cadence is small, incremental releases rather than long-lived stabilization branches.
- There is no LTS branch yet.
- Security or severe regression fixes should land in the next available release rather than waiting for a large batch.
