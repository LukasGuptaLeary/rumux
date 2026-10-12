# rumux

A Rust-native terminal workspace inspired by [cmux](https://github.com/manaflow-ai/cmux) that combines:

- A CLI for git worktree lifecycle management during parallel agent or shell sessions
- A GPUI desktop application for docked terminals, workspace switching, and session restore

The desktop shell uses Rust, GPUI and native docking primitives. Browser panes embed the operating system's web engine beside the GPU-rendered terminals.

## Highlights

- Git worktree orchestration for parallel development sessions
- Native desktop terminal UI built with GPUI
- Docked tabs, split panes, zoom, and multi-workspace sidebar
- Restorable workspace layout, terminal history and launch directories
- Local and SSH terminals keep running when the desktop closes
- Local RPC endpoint for automation and editor or agent integration
- MCP bridge for terminal and browser control over the running desktop app
- Embedded browser tabs and splits with navigation, DOM inspection and form interaction
- Backward compatibility with existing cmux `.worktrees/` layouts and setup hooks

## Install

Preferred CLI install on macOS and Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/LukasGuptaLeary/rumux/main/install.sh | sh
```

Install a specific release:

```bash
curl -fsSL https://raw.githubusercontent.com/LukasGuptaLeary/rumux/main/install.sh | sh -s -- --version v0.2.0
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

Desktop packages:

- `rumux-desktop-x86_64-unknown-linux-gnu.tar.gz`
- `rumux-desktop-aarch64-apple-darwin.zip`
- `rumux-desktop-x86_64-pc-windows-msvc.zip`
- `rumux-desktop-checksums.txt`

The [v0.2.0 release](https://github.com/LukasGuptaLeary/rumux/releases/tag/v0.2.0) packages the desktop and CLI together, with icons,
licenses, source metadata and checksums. Download a matching desktop archive
and `rumux-desktop-checksums.txt`, verify its checksum and extract it. From the
extracted package directory, install for your user:

```bash
# macOS: ~/Applications/Rumux.app; Linux: ~/.local/share/rumux
sh ./install-desktop.sh
# Optional isolated install location on either platform
sh ./install-desktop.sh --prefix /absolute/install/directory
```

```powershell
# Windows: %LOCALAPPDATA%\Rumux, user PATH and a Start-menu shortcut
powershell -ExecutionPolicy Bypass -File .\install-desktop.ps1
```

The installers verify the extracted files before installation. Close the
desktop and run `rumux session stop` before replacing running binaries. Config
and history live separately from the installation. Bundles are unsigned;
macOS notarization, Windows signing and distro-specific packages remain future
distribution work. Desktop installers for macOS, Linux and Windows ship in
`v0.2.0`; the older `v0.1.0` release contains CLI assets only.

The macOS bundle requires macOS 14 or newer. Windows requires DirectX 11,
WebView2 Runtime and the latest
[Visual C++ v14 x64 runtime](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/).
Linux CI packages target Ubuntu 24.04-compatible libraries; build from source
on older distributions.

Current desktop support policy lives in [COMPATIBILITY.md](COMPATIBILITY.md).

### Terminal appearance

The terminal uses a neutral dark background, a Catppuccin-derived ANSI palette,
and brighter comment/detail colors. Defaults are 15 px text, 1.25 line height
and 12 px pane padding. Keep your preferred monospace font and adjust spacing in
`config.toml`:

```toml
font_family = "Menlo" # macOS; Consolas on Windows, DejaVu Sans Mono on Linux
font_size = 15.0
line_height = 1.25
terminal_padding = 12.0
```

Reopen Rumux to apply font and spacing changes. Existing font preferences are
preserved. Syntax colors come from your shell, editor or command-line tools:
Rumux renders ANSI/256-color and true-color output, including bold, italic,
underline, dim, inverse and application palette overrides. It does not infer a
programming language from arbitrary terminal output.

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
| `rumux ssh [user@]host [--cwd /remote/path] [-L local:remote]` | Open an SSH workspace in the running desktop app |
| `rumux session status [--json]` | List processes owned by the profile's terminal service |
| `rumux session stop` | Stop the profile's terminal service and its processes, retaining history |

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

- **Rendering:** GPUI for the shell and terminal; Wry hosts WKWebView on macOS, WebKitGTK on Linux, and WebView2 on Windows.
- **Workspace shell:** `AppState -> Workspace -> DockArea -> TerminalPanel/BrowserPanel`
- **Docking model:** `gpui-component` dock/tree primitives drive tabbed terminals, pane splits, and zoom state.
- **Terminal runtime:** a per-profile headless service owns `portable-pty` processes and the authoritative `alacritty_terminal` parser; `gpui-terminal` renders parsed state in the desktop.
- **Persistence:** workspaces restore dock layout, pane IDs, launch directories and browser URLs. Stable terminal IDs reattach to the same service-owned processes; a cold service restart restores prior primary-screen history into fresh shells.
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
- `surface.*` methods focus, rename, close and lay out terminal or browser panes; terminal I/O methods require a terminal
- Selectors accept stable `workspace_id`/`surface_id` UUIDs as well as numeric indices; copies receive fresh identities
- `surface.send_text` with `append_newline` sends the same carriage return as the Enter key
- `system.activate` raises the primary desktop window, which is also what a second app launch uses for handoff
- `notification.focus` accepts a notification `id`, focuses its workspace and terminal, and marks it read
- Independent RPC clients can wait for output and send input concurrently; failed raw RPC calls exit with a nonzero CLI status
- The RPC contract is intended for local-machine use and is versioned conservatively within a release line, but it is still evolving during `0.1.x`

### Embedded browser

Use the globe button or Cmd/Ctrl+Shift+O to open a browser split. Enter an HTTP(S)
URL in the address bar. Back, forward and reload controls are available in each
pane. Browser IDs and URLs restore with the workspace; navigation history is
kept only while the pane is open.

```bash
rumux rpc browser.create '{"url":"http://localhost:3000","placement":"right"}'
# Use the created surface UUID from the response for later calls.
rumux rpc browser.snapshot '{"surface_id":"<uuid>"}'
rumux rpc browser.eval '{"surface_id":"<uuid>","expression":"document.title"}'
rumux rpc browser.fill '{"surface_id":"<uuid>","selector":"#name","text":"Ada"}'
rumux rpc browser.click '{"surface_id":"<uuid>","selector":"button[type=submit]"}'
```

The MCP bridge exposes the same browser actions as named tools. Evaluations
accept synchronous JavaScript expressions; errors and Promise results are
reported. Snapshots bound page text and interactive elements and omit password
field values. The page's IPC bridge only reports focus to the shell.

Linux uses X11 or XWayland when `DISPLAY` is available so WebKitGTK can embed a
native child view. `RUMUX_LINUX_BACKEND=wayland` selects the native Wayland
terminal shell; embedded browsers require XWayland. Install GTK 3 and
WebKitGTK 4.1 runtime libraries on Linux, and WebView2 Runtime on Windows.
Native browser layout, navigation and automation pass debug and installed-package
checks on all three platforms. See [COMPATIBILITY.md](COMPATIBILITY.md) for coverage.

### SSH workspaces

Choose **Connect SSH** in the command palette, or use the CLI:

```bash
rumux ssh me@dev-box --cwd /home/me/project --name "Remote project"
# Preview a service listening on the remote host's loopback interface
rumux ssh dev-box --cwd /srv/project --forward 3001:3000
rumux rpc browser.create '{"url":"http://127.0.0.1:3001"}'
```

OpenSSH must be on `PATH`. Hosts may be aliases from your SSH config. The dialog
has advanced fields for an identity, config and known-hosts file; the CLI also
accepts `--identity-file`, `--config-file`, `--known-hosts-file` and `--port`.
Authentication prompts appear in the terminal. Supplying a known-hosts file
enables strict host-key verification. Private keys and credentials are handled
by OpenSSH.

Remote POSIX shells export `TERM_PROGRAM=rumux` and their stable
`RUMUX_SURFACE_ID`. OSC 7 directory updates are retained for subsequent tabs and
splits. Connections, remote shells and forwards survive desktop exit while the
terminal service remains running. A cold service restart reconnects using the
saved connection and launch directory. A forwarded
listener belongs to its original pane; new tabs and workspace copies do not
bind it again, and closing that pane closes the listener. Forwarding binds only
local loopback. The `create_ssh_workspace` MCP tool accepts a typed `ssh` object,
also available through `workspace.create` RPC.

Real connections, forwarding and process survival pass native debug and installed
checks on macOS, Linux and Windows. Remote shells must accept POSIX command syntax.

### Terminal service and history

Quitting the desktop detaches its views and leaves terminal processes running.
Reopening the same profile reattaches to those processes, including active
alternate screens, colors and output produced while the desktop was absent.
Closing a pane or workspace explicitly stops its terminals and removes their
stored history. Browser views are recreated when the desktop reopens.

```bash
rumux session status --json
# Explicitly stop every terminal in the selected profile; reopen for fresh shells.
rumux session stop
```

Scrollback is saved under `terminals/` in the Rumux config directory every five
seconds when changed and when the service stops normally. A cold restart loads
bounded prior primary-screen history; it starts fresh processes and clears old
alternate-screen and input modes. Abrupt service or machine failure may lose
output since the last checkpoint. Unsupported or damaged history is preserved
and reported. An incompatible live service is preserved rather than replaced.

History can contain secrets printed by terminal programs. To disable disk
history for newly started panes, put `persist_scrollback = false` in
`config.toml`; existing processes retain the setting they started with. Live
memory scrollback still works. The service directory and files are restricted
to the current user (Unix permissions or Windows ACLs). Its loopback protocol
requires a private random token. The service supports 64 panes; each grid is
bounded to 512 columns, 256 rows and 1,000,000 cells including up to 10,000
history rows.

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
