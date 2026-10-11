# Cross-platform completion evidence

The October 11, 2026 completion goal remains active. The target is the core cmux
workflow on macOS, Linux and Windows, including browser and remote surfaces.
See [ROADMAP.md](../ROADMAP.md) for outstanding acceptance criteria.

## Terminal foundation checkpoint

Starting commit: `a342a59`. Working branch: `codex/complete-cross-platform-rumux`.
Architectural reference: [herdr](https://github.com/herdrdev/herdr), inspected at
`75f46fe812dbc2946f7e4104c7c89b70f5a84f0a`. No reference code was copied.

This batch connects scrollback, selection and search; handles fragmented OSC
notifications, terminal titles and directory reports; replies to terminal
queries; reaps PTY children; targets notification navigation at its originating
pane; serves concurrent bounded RPC clients; and makes session saves atomic,
with backup recovery and protection against overwriting future schemas.

Validation on October 11, 2026:

- macOS ARM64, Rust 1.97.0, GPUI runtime shaders: 116 unit/integration tests
  passed, plus 28 doctests (28 existing illustrative doctests ignored).
- Real macOS desktop smoke: all ten checks passed in 3.32 seconds.
- Linux ARM64 in Debian Bookworm, Rust 1.97.0: full workspace tests passed,
  desktop and CLI built, and all ten real desktop smoke checks passed in
  0.95 seconds under Xvfb and a session D-Bus.
- Linux source archive SHA-256:
  `6d75ab415028510ac86885754af0130cbaf7267b357d40e7b6cbe8fe6c3c1a8f`.
  Image manifest:
  `sha256:adc8caf47af9a6e5835afbeb234dc8aa9ffa1103b33267173b5ba5dbd67a4e5c`.
- Formatting, whitespace, Python syntax and shell syntax checks passed.
  Clippy completed with existing warnings; this is not a warning-free result.
- Native visual inspection verified two terminal splits and the search bar's
  match count and highlights on macOS.

The smoke script exercises actual PTYs, concurrent clients, hidden-pane OSC
notifications, exact notification focus, directory inheritance, MCP, workspace
removal, pane close, single-instance activation and layout/directory restart.
It uses isolated temporary configuration and runtime directories.

Windows, Linux x86-64 and macOS Intel have not been executed locally. The native
CI matrix is prepared but has not run. Compilation and smoke evidence above do
not establish packaging, detached process survival, browser or SSH acceptance.
At that checkpoint no branch, PR, release or third-party comment had been published.

## Workspace and automation checkpoint

The second batch adds stable persisted UUIDs to workspaces and terminal panes,
pins asynchronous RPC calls to their original target, gives duplicated panes
independent identities, and exposes explicit agent activity through RPC, MCP,
OSC 133 and the sidebar. Terminals export their own surface ID for CLI calls.
The worktree CLI supports a selected agent executable, direct arguments and
`--no-agent`, and propagates agent failures.

Loopback TCP now requires a random per-profile token, rotated at startup. Unix
token files are private (0600). The CLI and MCP bridge authenticate without
exposing tokens in output. Direct clients must adopt the authentication field.

Validation on macOS ARM64: 120 unit/integration tests plus 28 doctests passed;
13 real desktop smoke checks passed in 3.79 seconds over Unix sockets and 14
checks passed in 2.20 seconds over authenticated TCP. Linux ARM64 passed the
same full test barrier and both smoke suites, in 1.24 and 1.25 seconds.
The Linux source archive for this run has SHA-256
`11ee4baa86965d3f8a436b968255f1aae0a5fd591a3f8e48851d3d7fdd6d142c`.
The subsequent sidebar subscription correction was visually verified on macOS;
the final local barrier and smoke runs also include that correction. A final
focused CLI regression check rejects non-object RPC params before injecting a
terminal's origin ID, avoiding a malformed-input panic (121 tests total).

## Published checkpoint and native CI

With explicit user approval, commits `5804afa` and `c583941` were pushed to the
public `LukasGuptaLeary/rumux` repository and draft
[PR #9](https://github.com/LukasGuptaLeary/rumux/pull/9) was opened.
[CI run 38165297268](https://github.com/LukasGuptaLeary/rumux/actions/runs/38165297268)
is bound to `c5839417e9200c509fbb31b68db6faf3083c0266`.
Rustfmt, CLI/core, Linux x86-64 desktop tests/smokes, macOS desktop tests/smokes,
and Linux packaging passed. Windows compiled and passed workspace Rust tests,
then failed to observe an OSC 133 status signal in the real desktop smoke.
The browser/platform batch below changes RPC text submission to the same CR as
the Enter key and explicitly enables VT output in Windows fixture producers.
These corrections require Windows revalidation; compilation alone is not a
successful Windows acceptance result.

## Browser and platform batch

Browser panels host the system engine through Wry: WKWebView, WebKitGTK or
WebView2. They share the dock's tabs/splits and stable identities, restore URLs,
track page titles, offer back/forward/reload/address controls, and hide native
views for inactive tabs/workspaces, overlays and other zoomed panes. CLI RPC and
named MCP tools support navigation, state, bounded snapshots, synchronous
JavaScript, CSS-selector clicks and form input. Page IPC reports focus only.

The initial Linux attempts exposed two concrete integration defects: creating a
WebKit context before GTK initialization, then GPUI 0.2.2's unimplemented X11 raw
window handles. A subsequent attempt exposed Wry's requirement for an Xlib XID
rather than an XCB handle. Initialization is now lazy, and an adapter borrows the
same live X11 window ID. `crates/gpui` retains the published 0.2.2 source and
Apache-2.0 license with only two source files changed: native X11 handles and a
fallible explicit X11 constructor. Rumux selects X11/XWayland when available
without changing the environment inherited by terminal programs. See
`crates/gpui/RUMUX-PATCHES.md` for provenance and the patch removal boundary.

Final local validation on October 11, 2026:

- macOS ARM64 and Linux ARM64: 123 workspace tests plus 28 doctests passed on
  each platform (28 existing illustrative doctests ignored).
- macOS native browser/PTY/CLI/MCP integration: 19 Unix-socket checks in 4.76 s
  and 20 authenticated-TCP checks in 3.63 s.
- Linux native browser/PTY/CLI/MCP integration: 19 checks in 3.30 s and 20 TCP
  checks in 3.15 s, with the normal WebKit sandbox. The XWayland selection case
  also passed all 19 checks in 3.15 s with a Wayland session variable present.
- Linux source archive SHA-256:
  `ee2d00c4e1a469797a07d722e02154a6d4c55b95df4d1dd0437faa90a88c0b0f`.
  Browser validation image manifest:
  `sha256:0f20c67a76a6da29474bcba1c914164996d2464fea2089da930d9f3e267ee987`.
- Native macOS inspection confirmed terminal/browser bounds, real input and
  button clicks, command-palette hiding, and browser zoom.
- Formatting, whitespace, Python/shell syntax checks passed. Clippy completed
  with existing warnings; the vendored GPUI source also has upstream float
  inference warnings on Rust 1.97.

The smoke suite additionally proves URL/ID restart restore, mixed pane routing,
form values containing quotes/Unicode, evaluation errors, rejected unsupported
navigation, Promise rejection and password-field omission in DOM snapshots.
The CI matrix now includes browser tests on all three platforms. Browser
behavior on Windows and the proposed Enter-key correction remain unqualified
until this batch runs there. Native Wayland browser hosting is unsupported;
Linux browser panes use X11/XWayland. Browser navigation history is not restored,
and JavaScript evaluation is synchronous.

## Next acceptance boundaries

Durable terminal ownership, Windows browser qualification, SSH workflows and three-platform installation proof
remain required. Any product claim must distinguish configured gates from
executed results, and restored layout from a still-running terminal process.
