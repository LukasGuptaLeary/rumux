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

The core completion goal is qualified by native debug and installed checks on
all three platforms at `32b0ad3`. See [COMPATIBILITY.md](COMPATIBILITY.md) and
[the evidence log](docs/development-progress.md). The branch and draft PR contain
the completed source; merge and release publication remain separate owner actions.

| Milestone | Acceptance | Status |
|---|---|---|
| Terminal foundation | Scrollback, selection, paste, find, resize, terminal query replies, title/CWD updates and process cleanup work in real PTYs | Qualified on all three OSes |
| Workspace and agent workflow | Stable identities, metadata, pane notifications, unread navigation, directory inheritance, configurable commands and agent-independent CLI | Implemented and qualified |
| Session lifecycle | Atomic versioned saves, recovery, layout/CWD/scrollback restore, and explicit detach/reattach without silent process loss | Qualified, including Windows ACLs and SSH process survival |
| Browser and remote surfaces | Embedded browser beside terminal, navigation and agent automation; SSH workspaces and remote directory/port routing | Qualified on macOS, Windows and Linux X11/XWayland |
| Cross-platform distribution | Desktop packages for all three OSes; isolated installation and startup/RPC checks | Qualified unsigned archives and per-user installers |
| Completion qualification | Native checks and install evidence identify one source commit; limitations documented; authorized branch/PR publication | All eight gates passed; desktop release unpublished |

Working branch: `codex/complete-cross-platform-rumux`, starting at `a342a59`.
The starting CLI/core baseline passes 30 tests. The first desktop build exposed
a missing local Metal compiler; use GPUI's supported runtime shader feature for
local source validation and keep precompiled shaders for release builds.

## Near Term

- Harden desktop packaging beyond raw archives:
  - Add macOS signing and notarization
  - Add Windows signing
  - Add Linux-native packaging targets such as `.deb` or AppImage once runtime support is clear
- Broaden interactive Windows coverage to alternative shells and display configurations
- Tighten release engineering:
  - Publish a qualified desktop release after owner review and authorization
  - Document platform prerequisites more precisely

## Product Parity

- Continue improving cmux workflow ergonomics
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
- Establish a more formal release cadence and support policy

## Open Source Project Health

- Add screenshots and short demo media to the README and release notes
- Build a contributor-friendly issue taxonomy for UX, platform, and workflow work
- Track breaking changes and migration notes more explicitly in the changelog
- Publish architectural decision records once the desktop shell and RPC model settle
