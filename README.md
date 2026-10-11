# rumux

A Rust-native replacement for [cmux](https://github.com/craigsc/cmux) that combines:

- A CLI for git worktree lifecycle management during parallel agent or shell sessions
- A GPUI desktop application for docked terminals, workspace switching, and session restore

The desktop shell follows the same architectural direction as Zed: pure Rust, GPUI, and native docking primitives rather than a webview stack.

## Highlights

- Git worktree orchestration for parallel development sessions
- Native desktop terminal UI built with GPUI and wgpu
- Docked tabs, split panes, zoom, and multi-workspace sidebar
- Restorable workspace layout and terminal launch directories
- Local RPC endpoint for automation and editor or agent integration
- MCP bridge for agent control over the running desktop app
- Backward compatibility with existing cmux `.worktrees/` layouts and setup hooks

## Install

Preferred CLI install on macOS and Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/LukasGuptaLeary/rumux/main/install.sh | sh
```

Install a specific release:

```bash
curl -fsSL https://raw.githubusercontent.com/LukasGuptaLeary/rumux/main/install.sh | sh -s -- --version v0.1.0
```

Source install fallback:

```bash
cargo install --git https://github.com/LukasGuptaLeary/rumux.git rumux-cli --bin rumux
```

Notes:

- The installer currently publishes prebuilt CLI binaries for macOS (`aarch64`) and Linux (`x86_64`)
- Windows users should download the CLI asset directly from GitHub Releases for now
- Intel macOS users should use the source install path for now
- Git available on `PATH` for normal `rumux` usage
- Rust 1.88 or newer is required for source installs and local builds (including locked dependencies)
- For the desktop app, the native graphics and windowing libraries required by GPUI on your platform

Desktop app build and run:

```bash
cargo run -p rumux-app
```

On macOS with Command Line Tools but no Metal compiler, GPUI can compile the
same shaders at runtime:

```bash
cargo run -p rumux-app --features runtime-shaders
```

On Linux, expect to need the usual development headers for Wayland and/or X11, `xkbcommon`, `fontconfig`, and `dbus` before building the desktop app.

Desktop release artifacts:

- `rumux-desktop-x86_64-unknown-linux-gnu.tar.gz`
- `rumux-desktop-aarch64-apple-darwin.zip`
- `rumux-desktop-checksums.txt`

Those assets ship the current desktop binary in a release-ready archive format. They are not yet native installers, signed bundles, or distro packages.

Current desktop support policy lives in [COMPATIBILITY.md](COMPATIBILITY.md).

## Usage

| Command | Description |
|---------|-------------|
| `rumux new <branch> [-p <prompt>] [--agent <program>] [--no-agent]` | Create a worktree, run setup and optionally launch an agent |
| `rumux start <branch> [--agent <program>] [--no-agent]` | Launch an agent in an existing worktree |
| `rumux cd [branch]` | Print worktree path (repo root if no branch given) |
| `rumux ls` | List active worktrees |
| `rumux merge [branch] [--squash]` | Merge a worktree branch into the current branch |
| `rumux rm [branch] [--all] [--force]` | Remove a worktree and its branch |
| `rumux init [--replace]` | Generate a `.rumux/setup` hook using Claude |
| `rumux update` | Show update instructions |
| `rumux version` | Print version |
| `rumux completions <shell>` | Generate shell completions (bash, zsh, fish, powershell) |
| `rumux mcp` | Run a stdio MCP server for a running `rumux-app` instance |

## Typical Workflow

```bash
# Create a new worktree and launch Claude Code
rumux new feature-auth

# In another terminal, create a second parallel session
rumux new feature-api -p "implement the REST API endpoints"

# List active worktrees
rumux ls

# Resume a previous session
rumux start feature-auth

# Merge completed work back
rumux merge feature-auth

# Clean up
rumux rm feature-auth
```

## Desktop App

rumux includes a pure-Rust desktop application built on GPUI.

### Architecture

- **Rendering:** GPUI + wgpu. No Tauri, React, xterm.js, or browser runtime.
- **Workspace shell:** `AppState -> Workspace -> DockArea -> TerminalPanel -> TerminalView`
- **Docking model:** `gpui-component` dock/tree primitives drive tabbed terminals, pane splits, and zoom state.
- **Terminal runtime:** `portable-pty` + `alacritty_terminal` + `gpui-terminal`
- **Persistence:** workspaces restore dock layout and terminal launch directories across restarts.
- **IPC:** Unix domain sockets on Unix by default, loopback TCP on non-Unix. Override with `RUMUX_SOCKET_PATH` or `RUMUX_SOCKET_ADDR`.
- **Instance model:** one desktop app instance per user-scoped IPC endpoint. A second launch activates the primary instance and exits.

### Features

- Multiple workspaces with a vertical sidebar
- Docked terminal tabs and split panes
- Session save and restore for workspace layout
- Command palette, notification panel, and find overlay
- Ghostty-compatible terminal font configuration
- Scrollback with mouse/trackpad scrolling, literal find with smart case and match navigation
- OSC 7 directory tracking and OSC 0/2 terminal titles
- Streaming OSC 9/99/777 notifications, unread pane borders and click-to-focus navigation
- Local RPC endpoint for automation and integration
- Single-instance desktop app with launch handoff to the running window

### Desktop RPC

The desktop app exposes a local JSON-RPC endpoint for editor and agent integration.

Examples:

```bash
rumux capabilities
rumux rpc system.activate
rumux rpc workspace.list
rumux rpc surface.list
rumux rpc surface.send_text '{"text":"cargo test","append_newline":true}'
rumux rpc surface.send_key '{"keystroke":"ctrl-c"}'
rumux notify --title "Ready for review" --body "Tests passed" --workspace 0 --surface 1
```

Current automation model:

- `workspace.*` methods let integrations inspect, create, select, and close workspaces
- `surface.*` methods operate on the current target terminal by default
- `surface.*` methods can also target a specific terminal via `surface_index`
- `system.activate` raises the primary desktop window, which is also what a second app launch uses for handoff
- `notification.focus` accepts a notification `id`, focuses its workspace and terminal, and marks it read
- Independent RPC clients can wait for output and send input concurrently; failed raw RPC calls exit with a nonzero CLI status
- The RPC contract is intended for local-machine use and is versioned conservatively within a release line, but it is still evolving during `0.1.x`

### MCP Integration

The CLI can expose the running desktop app as a stdio MCP server:

```bash
rumux mcp
```

The MCP bridge is intentionally thin:

- It exposes typed MCP tools for the current desktop RPC surface
- It includes an `rpc_raw` tool as an escape hatch for advanced integrations
- It controls the already-running `rumux-app`; it does not spin up a second desktop instance

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Ctrl/Cmd+Shift+N | New workspace |
| Ctrl/Cmd+Shift+W | Close workspace |
| Ctrl/Cmd+Shift+T or Cmd+T | New terminal |
| Ctrl/Cmd+Shift+X or Cmd+W | Close terminal |
| Ctrl+Shift+D or Cmd+D | Split right |
| Ctrl+Alt+D or Cmd+Shift+D | Split down |
| Ctrl/Cmd+Shift+P | Command palette |
| Ctrl/Cmd+Shift+I | Notification panel |
| Ctrl/Cmd+F | Find in terminal |
| Ctrl/Cmd+B | Toggle sidebar |
| Ctrl/Cmd+Q | Quit |

## Agent control and profiles

For testing separate desktop profiles, set `RUMUX_CONFIG_DIR` and
`RUMUX_RUNTIME_DIR` together with a unique `RUMUX_SOCKET_PATH` (Unix) or
`RUMUX_SOCKET_ADDR` (loopback TCP). Session saves are atomic with a valid-state
backup. Damaged or unsupported future sessions are preserved and reported.

Use `--agent codex` (or another executable) and repeat `--agent-arg` to pass its
arguments directly. `RUMUX_AGENT` sets the CLI default. The compatibility default
is Claude: `new --prompt` uses `-p`, and `start` uses `--continue` when no custom
arguments are given. Other programs receive the prompt as one final argument;
their resume flags belong in `--agent-arg`. `--no-agent` prints the prepared
worktree path. Set `agent_command = "codex"` in `config.toml` to change the
terminal toolbar's launch command.

Workspace and surface summaries include stable UUIDs in `id`. Prefer
`workspace_id` and `surface_id` selectors for automation; numeric selectors
remain available. IDs survive restart and copies receive fresh IDs. A
`surface_id` alone finds its workspace. Asynchronous output waits stay bound to
the original surface when focus or workspace order changes. Terminals export
`RUMUX_SURFACE_ID`, which the CLI uses as its default surface target.

Agents can report activity with `surface.set_status`, for example
`{"surface_id":"<uuid>","status":"blocked","message":"Review requested"}`.
The MCP bridge exposes the same operation. Supported states are `idle`,
`working` and `blocked`; process exit sets `exited`. OSC 133 prompt/command
markers update activity when provided by shell integration.

Loopback TCP requires a per-profile token in `rpc-<port>.token` under the Rumux
config directory. The CLI and MCP bridge read it automatically. Direct TCP
clients must include `auth_token` in each request; the server rotates it at
startup. Unix sockets use filesystem access permissions. The legacy numeric
`workspace_id` in notification responses remains an index; use workspace and
surface summary `id` values for stable targeting.

## Setup Hooks

rumux can automatically set up new worktrees (install dependencies, symlink config files, etc.) by running a hook script.

### Generate a hook automatically

```bash
rumux init
```

This uses Claude Code to analyze your repo and generate a `.rumux/setup` script. Review the generated script before committing it.

### Manual setup

Create `.rumux/setup` in your repo root:

```bash
#!/bin/bash
REPO_ROOT="$(git rev-parse --git-common-dir | xargs dirname)"

# Install dependencies
npm install

# Symlink environment files
ln -sf "$REPO_ROOT/.env" .env

# Run codegen
npm run generate
```

Make it executable:

```bash
chmod +x .rumux/setup
```

The hook runs automatically in each new worktree created by `rumux new`.

## Shell `cd` Integration

Since a compiled binary cannot change the parent shell's directory, add this function to your shell config (`.bashrc`, `.zshrc`, etc.):

```bash
rumux() {
  if [ "$1" = "cd" ]; then
    local dir
    dir=$(command rumux cd "${@:2}") && cd "$dir"
  else
    command rumux "$@"
  fi
}
```

Then `rumux cd feature-foo` will change your shell's directory to the worktree.

## Shell Completions

Generate completions for your shell:

```bash
# Bash
rumux completions bash > ~/.local/share/bash-completion/completions/rumux

# Zsh
rumux completions zsh > ~/.zfunc/_rumux

# Fish
rumux completions fish > ~/.config/fish/completions/rumux.fish
```

## How It Works

rumux manages git worktrees under a `.worktrees/` directory in your repo root. Each worktree gets its own branch, working directory, and isolated file state, which makes it practical to run multiple agent or shell sessions without conflicts.

- **Worktrees** are stored in `<repo_root>/.worktrees/<branch>/`
- **Branches** are sanitized: `feature/foo` becomes `feature-foo`
- **Setup hooks** run in each new worktree to install dependencies and configure the environment
- **Backward compatible** with cmux: existing `.worktrees/` directories and `.cmux/setup` hooks work out of the box

## Backward Compatibility

rumux is backward compatible with cmux:

- Worktrees are stored in the same `.worktrees/` location
- If `.cmux/setup` exists but `.rumux/setup` does not, the legacy hook is used (with a note to rename it)

## Development

- [CONTRIBUTING.md](CONTRIBUTING.md) for setup, workflow, and testing expectations
- [SECURITY.md](SECURITY.md) for vulnerability reporting
- [SUPPORT.md](SUPPORT.md) for support boundaries and issue routing
- [RELEASING.md](RELEASING.md) for release steps
- [COMPATIBILITY.md](COMPATIBILITY.md) for platform support and upgrade guarantees
- [ROADMAP.md](ROADMAP.md) for current product and release priorities
- [CLAUDE.md](CLAUDE.md) for the current architecture map and build notes used in this repo
- [install.sh](install.sh) for the release-based CLI installer
- [.github/workflows/release-desktop.yml](.github/workflows/release-desktop.yml) and [scripts/package-desktop.sh](scripts/package-desktop.sh) for desktop release packaging

## License

rumux source is available under the MIT license. See [LICENSE](LICENSE).

This repository also vendors third-party crates that retain their original licenses. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for details.
