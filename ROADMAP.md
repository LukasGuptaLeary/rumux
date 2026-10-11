# Roadmap

This roadmap tracks the highest-value work for turning `rumux` into a stable open source terminal workspace platform.

## Completion goal (October 11, 2026)

Deliver a native terminal workspace application on macOS, Linux and Windows
with the core cmux workflow: workspace sidebar, tabs and splits, terminal and
browser surfaces, notifications that focus the originating pane, persistence,
keyboard control, and a CLI/RPC/MCP interface for coding agents.
Use [herdr](https://github.com/herdrdev/herdr) as an architectural reference for
PTY ownership, bounded streaming protocols, session lifecycle, remote work and
agent status. Preserve Rumux's GPUI shell and worktree CLI.

The completion goal remains open until the acceptance checks below have evidence.
Successful compilation on macOS does not establish Linux or Windows support.

| Milestone | Acceptance | Status |
|---|---|---|
| Terminal foundation | Scrollback, selection, paste, find, resize, terminal query replies, title/CWD updates and process cleanup work in real PTYs | Validated on macOS/Linux; Windows pending |
| Workspace and agent workflow | Stable identities, metadata, pane notifications, unread navigation, directory inheritance, configurable commands and agent-independent CLI | Implemented; macOS/Linux proof recorded |
| Session lifecycle | Atomic versioned saves, recovery, layout/CWD/scrollback restore, and an explicit detach/reattach lifecycle without silent process loss | Pending |
| Browser and remote surfaces | Embedded browser beside terminal, navigation and agent automation; SSH workspaces and remote directory/port routing | Pending |
| Cross-platform distribution | Build/test gates and desktop packages for all three OSes; fresh installation and startup/RPC smoke checks | Pending |
| Release qualification | Tests, live UI/PTY checks and install evidence match one commit; limitations documented; user authorizes external publication | Pending |

Working branch: `codex/complete-cross-platform-rumux`, starting at `a342a59`.
The starting CLI/core baseline passes 30 tests. The first desktop build exposed
a missing local Metal compiler; use GPUI's supported runtime shader feature for
local source validation and keep precompiled shaders for release builds.

## Near Term

- Harden desktop packaging beyond raw archives:
  - Add app icons and brand assets
  - Add macOS signing and notarization
  - Add Linux-native packaging targets such as `.deb` or AppImage once runtime support is clear
- Validate desktop support on Windows instead of treating it as CLI-only:
  - Get `rumux-app` compiling and launching in CI on Windows
  - Identify any GPUI, PTY, or clipboard gaps
- Tighten release engineering:
  - Attach desktop artifacts to tagged releases automatically
  - Add smoke-test coverage for the packaging script and desktop startup path
  - Document platform prerequisites more precisely

## Product Parity

- Reach full feature parity with legacy `cmux` workflows where they still matter
- Expand terminal ergonomics:
  - Better search UX
  - Richer tab management actions
  - More polished focus and selection behavior
- Improve automation surfaces:
  - Flesh out RPC-driven terminal control
  - Stabilize editor and agent integration paths

## Enterprise Readiness

- Add clearer configuration layering and policy controls
- Improve observability around workspace/session failures
- Define migration and compatibility guarantees for on-disk session state
- Add CI coverage across Linux, macOS, and Windows for both CLI and desktop
- Establish a more formal release cadence and support policy

## Open Source Project Health

- Add screenshots and short demo media to the README and release notes
- Build a contributor-friendly issue taxonomy for UX, platform, and workflow work
- Track breaking changes and migration notes more explicitly in the changelog
- Publish architectural decision records once the desktop shell and RPC model settle
