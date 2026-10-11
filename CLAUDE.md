# rumux — Developer Guide

Rust rewrite of cmux. Includes a CLI (worktree lifecycle manager) and a GPU-accelerated desktop terminal app (GPUI).

## Build & Test

```bash
# CLI only
cargo build -p rumux-cli
cargo test -p rumux-cli -p rumux-core

# Desktop app
cargo build -p rumux-app
cargo build -p rumux-app --release

# Full workspace
cargo build --workspace
cargo test --workspace
cargo clippy --workspace

# macOS without a precompiled Metal toolchain
cargo test --workspace --features rumux-app/runtime-shaders --locked

# Real desktop/PTY/CLI/MCP integration (isolated configuration)
python3 scripts/smoke-desktop.py

# Embedded browser and real loopback SSH integration (requires OpenSSH server)
python3 scripts/smoke-desktop.py --browser --ssh

# Install from the current checkout
cargo install --path crates/rumux-cli
cp target/release/rumux-app ~/.cargo/bin/
```

## Workspace Structure

```
rumux/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── rumux-cli/                # CLI binary (`rumux`)
│   │   ├── src/main.rs           # Clap App, subcommand dispatch
│   │   ├── src/commands/         # One file per subcommand
│   │   ├── src/hook.rs           # Setup hook discovery and execution
│   │   └── src/shell.rs          # Configurable agent detection and launch
│   ├── rumux-core/               # Shared library
│   │   ├── src/config.rs         # Repo root detection, path helpers, branch sanitization
│   │   ├── src/errors.rs         # Typed error definitions (thiserror)
│   │   └── src/git_ops.rs        # All git2-based operations
│   └── gpui-terminal/            # Vendored terminal component (patched)
│       └── src/                  # TerminalView, renderer, VTE, input handling
├── app/                          # GPUI desktop app
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs               # App entry, window creation, keybindings
│       ├── app_state.rs          # Global state: workspaces, persistence wiring
│       ├── root_view.rs          # Top-level view: sidebar + active workspace + overlays
│       ├── sidebar.rs            # Vertical workspace sidebar with rename/select controls
│       ├── terminal_panel.rs     # Dock-compatible terminal panel
│       ├── workspace.rs          # Workspace entity backed by DockArea
│       ├── terminal_surface.rs   # PTY spawning + TerminalView creation
│       ├── command_palette.rs    # Fuzzy command palette overlay (Ctrl+Shift+P)
│       ├── notifications.rs      # Notification model
│       ├── socket_server.rs      # Local RPC transport (Unix socket or loopback TCP)
│       ├── session.rs            # Session save/restore
│       └── theme.rs              # Catppuccin Mocha color palette + UI colors
```

## Architecture

### CLI (`rumux`)
Single binary built with clap (derive API). All git operations use `git2` (libgit2). Agent executables are selected by `--agent` or `RUMUX_AGENT`; arguments are passed directly with `Command`. `--no-agent` supports worktree preparation without an agent installed. Hook generation still uses Claude.

### Desktop App (`rumux-app`)
Pure Rust, GPU-accelerated via GPUI framework (from Zed).

- **Rendering**: GPUI shell/terminal (Vulkan on Linux, Metal on macOS, DirectX on Windows). Browser panels use Wry with the system web engine.
- **Terminal**: `gpui-terminal` crate wraps `alacritty_terminal` for VTE parsing + GPU cell rendering. `portable-pty` for PTY I/O.
- **State model**: GPUI entities — AppState → Workspace → DockArea → TerminalPanel → TerminalView
- **Layout**: Dock/tree model from `gpui-component`, mirroring the architectural style used by Zed
- **Session restore**: Dock layout, stable pane IDs, terminal launch directories, browser URLs and typed SSH connections are serialized per workspace. SSH reconnects; process survival is pending.
- **Socket API**: local RPC over Unix sockets on Unix and loopback TCP on non-Unix
- **Notifications**: OSC 777/99/9 escape sequences parsed from PTY output

The legacy `v1` desktop path has been removed. Files under `app/src/` are the canonical shell.

### Vendored GPUI

`crates/gpui` pins published GPUI 0.2.2 with a narrow X11 native-handle patch and an explicit X11 constructor for XWayland browser hosting. Preserve its Apache-2.0 license and see `crates/gpui/RUMUX-PATCHES.md` before changing it. Browser tests use local fixtures via `python3 scripts/smoke-desktop.py --browser`.

### Vendored gpui-terminal
The `crates/gpui-terminal/` directory is a vendored copy of https://github.com/zortax/gpui-terminal. Rumux patches include palette rendering, scrollback and selection, literal search, terminal protocol replies, bounded output delivery and pane lifecycle hooks. It is a workspace member so its tests run in the full local barrier. Keep changes focused and preserve the upstream license. The streaming OSC parser lives in `rumux-core::terminal_protocol`.

## Conventions

- **Error handling**: `anyhow` for app errors, `thiserror` for typed errors. No `unwrap()` or `expect()` in non-test code.
- **Output (CLI)**: User-facing messages to stderr, machine-readable output to stdout.
- **Git operations**: Always `git2`, never shell out to `git`.
- **GPUI patterns**: Entities for state, `Render` trait for views, actions for keybindings, `cx.listener()` for event handlers.

## Key Dependencies

- `gpui` — GPU-accelerated UI framework
- `gpui-terminal` — Terminal emulator component (vendored)
- `alacritty_terminal` — VTE parsing + terminal grid state
- `portable-pty` — Cross-platform PTY management
- `smol` / `futures-lite` — Async runtime for socket server
- `clap` / `clap_complete` — CLI parsing + shell completions
- `git2` — libgit2 bindings
- `anyhow` / `thiserror` — Error handling

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Ctrl+Shift+P | Command palette |
| Ctrl+Shift+N | New workspace |
| Ctrl+Shift+W | Close workspace |
| Ctrl+Shift+D | Split right |
| Ctrl+Alt+D | Split down |
| Ctrl+Shift+T | New terminal |
| Ctrl+Shift+X | Close terminal |
| Ctrl+Tab | Next workspace |
| Ctrl+Shift+Tab | Previous workspace |
| Ctrl+PageDown | Next terminal tab |
| Ctrl+PageUp | Previous terminal tab |
| Ctrl++/- | Font size |
