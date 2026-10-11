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
No branch, PR, release or third-party comment has been published.

## Next acceptance boundaries

Stable workspace/pane identities and agent metadata precede durable terminal
ownership. Browser and SSH workflows and three-platform installation proof
remain required. Any product claim must distinguish configured gates from
executed results, and restored layout from a still-running terminal process.
