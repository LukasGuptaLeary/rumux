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
- Backward compatibility with existing cmux `.worktrees/` layouts and setup hooks

## Install

Current installation is source-first:

```bash
cargo install --path crates/rumux-cli
```

Requirements:

- Rust 1.85 or newer
- Git available on `PATH`
- For the desktop app, the native graphics and windowing libraries required by GPUI on your platform

Desktop app build and run:

```bash
cargo run -p rumux-app
```

On Linux, expect to need the usual development headers for Wayland and/or X11, `xkbcommon`, `fontconfig`, and `dbus` before building the desktop app.

## Usage

| Command | Description |
|---------|-------------|
| `rumux new <branch> [-p <prompt>]` | Create a new worktree + branch, run setup hook, launch Claude Code |
| `rumux start <branch>` | Resume Claude Code in an existing worktree |
| `rumux cd [branch]` | Print worktree path (repo root if no branch given) |
| `rumux ls` | List active worktrees |
| `rumux merge [branch] [--squash]` | Merge a worktree branch into the current branch |
| `rumux rm [branch] [--all] [--force]` | Remove a worktree and its branch |
| `rumux init [--replace]` | Generate a `.rumux/setup` hook using Claude |
| `rumux update` | Show update instructions |
| `rumux version` | Print version |
| `rumux completions <shell>` | Generate shell completions (bash, zsh, fish, powershell) |

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

### Features

- Multiple workspaces with a vertical sidebar
- Docked terminal tabs and split panes
- Session save and restore for workspace layout
- Command palette, notification panel, and find overlay
- Ghostty-compatible terminal font configuration
- Local RPC endpoint for automation and integration

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
- [CLAUDE.md](CLAUDE.md) for the current architecture map and build notes used in this repo

## License

rumux source is available under the MIT license. See [LICENSE](LICENSE).

This repository also vendors third-party crates that retain their original licenses. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for details.
