# Roadmap

This roadmap tracks the highest-value work for turning `rumux` into a stable open source terminal workspace platform.

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
