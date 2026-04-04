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

# Install
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
│   │   └── src/shell.rs          # Claude CLI detection and launch
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
│       ├── root_view.rs          # Top-level view: sidebar + workspace + overlays
│       ├── app_state.rs          # Global state: workspaces, active index
│       ├── workspace.rs          # Workspace entity: split tree layout, pane management
│       ├── pane.rs               # Pane entity: terminal tabs, tab bar, actions
│       ├── sidebar.rs            # Vertical workspace sidebar with tabs
│       ├── terminal_surface.rs   # PTY spawning + TerminalView creation
│       ├── command_palette.rs    # Fuzzy command palette overlay (Ctrl+Shift+P)
│       ├── notifications.rs      # OSC 777/99/9 parser
│       ├── socket_server.rs      # Unix socket JSON-RPC server (/tmp/rumux.sock)
│       ├── session.rs            # Session save/restore
│       └── theme.rs              # Catppuccin Mocha color palette + UI colors
```

## Architecture

### CLI (`rumux`)
Single binary built with clap (derive API). All git operations use `git2` (libgit2). The only `Command` usage is for launching `claude` and running setup hooks.

### Desktop App (`rumux-app`)
Pure Rust, GPU-accelerated via GPUI framework (from Zed).

- **Rendering**: GPUI + wgpu (Vulkan on Linux, Metal on macOS). No webview, no JavaScript.
- **Terminal**: `gpui-terminal` crate wraps `alacritty_terminal` for VTE parsing + GPU cell rendering. `portable-pty` for PTY I/O.
- **State model**: GPUI entities — AppState → Workspace (split tree) → Pane (terminal tabs) → TerminalView
- **Layout**: Recursive SplitTree (horizontal/vertical) with flex-based rendering
- **Socket API**: smol-based Unix socket at `/tmp/rumux.sock`, JSON-RPC protocol
- **Notifications**: OSC 777/99/9 escape sequences parsed from PTY output

### Vendored gpui-terminal
The `crates/gpui-terminal/` directory is a vendored copy of https://github.com/zortax/gpui-terminal with one patch: the div background color uses the terminal palette instead of a hardcoded value.

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
