# rumux — Developer Guide

Rust rewrite of cmux. Includes a CLI (worktree lifecycle manager) and a desktop terminal app (Tauri v2).

## Build & Test

```bash
# CLI only
cargo build -p rumux-cli
cargo test -p rumux-cli -p rumux-core

# Desktop app
cd app/src && pnpm install && pnpm test
cargo tauri build

# Full workspace
cargo build --workspace
cargo test --workspace
cargo clippy --workspace
```

## Workspace Structure

```
rumux/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── rumux-cli/              # CLI binary (`rumux`)
│   │   ├── src/main.rs         # Clap App, subcommand dispatch
│   │   ├── src/commands/       # One file per subcommand
│   │   ├── src/hook.rs         # Setup hook discovery and execution
│   │   └── src/shell.rs        # Claude CLI detection and launch
│   └── rumux-core/             # Shared library
│       ├── src/config.rs       # Repo root detection, path helpers, branch sanitization
│       ├── src/errors.rs       # Typed error definitions (thiserror)
│       └── src/git_ops.rs      # All git2-based operations
├── app/
│   ├── src-tauri/              # Tauri v2 Rust backend
│   │   ├── src/main.rs         # Tauri entry point
│   │   ├── src/pty.rs          # PTY management (portable-pty)
│   │   ├── src/ipc.rs          # Tauri IPC command definitions
│   │   ├── src/socket_server.rs # Unix socket API server
│   │   ├── src/socket_handler.rs # JSON-RPC method dispatch
│   │   ├── src/notifications.rs # OSC notification parser
│   │   ├── src/session.rs      # Session save/restore
│   │   ├── src/config.rs       # Ghostty-compat config loading
│   │   ├── src/commands.rs     # Custom command loading (cmux.json)
│   │   ├── src/browser.rs      # Browser automation handler
│   │   ├── src/sidebar.rs      # Sidebar metadata types
│   │   └── src/state.rs        # App state (PTY manager)
│   └── src/                    # Frontend (React + TypeScript + Vite)
│       ├── src/App.tsx
│       ├── src/components/     # Sidebar, PaneContainer, TerminalSurface, etc.
│       ├── src/stores/         # Zustand stores (workspace, notifications, settings)
│       ├── src/hooks/          # useTerminal, useSplitPane, usePty
│       └── src/lib/            # IPC wrappers, keybindings, theme
```

## Architecture

### CLI (`rumux`)
Single binary built with clap (derive API). All git operations use `git2` (libgit2). The only `Command` usage is for launching `claude` and running setup hooks.

### Desktop App (`rumux-app`)
Tauri v2 backend + React/xterm.js frontend.

- **PTY pipeline**: `portable-pty` spawns shells, background threads stream bytes to frontend via Tauri events, frontend writes to xterm.js WebGL terminal
- **State model**: Zustand stores on frontend: Window → Workspace → Pane (split tree) → Surface (terminal or browser)
- **Socket API**: Unix socket at `/tmp/rumux.sock`, JSON-RPC protocol, methods for workspace/surface/notification/sidebar control
- **Notifications**: OSC 777/99/9 escape sequences parsed from PTY output, forwarded to OS notifications via Tauri plugin

## Conventions

- **Error handling**: `anyhow` for app errors, `thiserror` for typed errors. No `unwrap()` or `expect()` in non-test code (except Tauri's `run().expect()` in main).
- **Output (CLI)**: User-facing messages to stderr, machine-readable output to stdout.
- **Git operations**: Always `git2`, never shell out to `git`.
- **Branch sanitization**: `/` → `-`, strip leading/trailing `-`, collapse `--`.
- **Frontend**: TypeScript strict mode, Zustand for state, all Tauri IPC calls wrapped with typed functions.

## Key Dependencies

### Rust
- `clap` / `clap_complete` — CLI parsing + shell completions
- `git2` — libgit2 bindings
- `anyhow` / `thiserror` — Error handling
- `tauri` v2 — Desktop app framework
- `portable-pty` — PTY management
- `tokio` — Async runtime (socket server)

### Frontend
- `@xterm/xterm` + addons (webgl, fit, search, serialize, unicode11, web-links)
- `react` + `zustand` — UI + state management
- `@tauri-apps/api` — Tauri IPC bridge
- `vite` + `vitest` — Build + test
