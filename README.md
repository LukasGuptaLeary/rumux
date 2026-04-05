# rumux

A git worktree lifecycle manager for running parallel Claude Code sessions. Rust rewrite of [cmux](https://github.com/craigsc/cmux).

## Install

```bash
cargo install rumux
```

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

## Setup Hook

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

rumux manages git worktrees under a `.worktrees/` directory in your repo root. Each worktree gets its own branch, working directory, and isolated file state — perfect for running multiple Claude Code sessions without conflicts.

- **Worktrees** are stored in `<repo_root>/.worktrees/<branch>/`
- **Branches** are sanitized: `feature/foo` becomes `feature-foo`
- **Setup hooks** run in each new worktree to install dependencies and configure the environment
- **Backward compatible** with cmux: existing `.worktrees/` directories and `.cmux/setup` hooks work out of the box

## Backward Compatibility

rumux is backward compatible with cmux:

- Worktrees are stored in the same `.worktrees/` location
- If `.cmux/setup` exists but `.rumux/setup` does not, the legacy hook is used (with a note to rename it)

## Desktop App

rumux includes a pure-Rust desktop application built on GPUI, following the same architectural direction as Zed rather than a webview stack.

### Current Architecture

- **Rendering:** GPUI + wgpu. No Tauri, React, xterm.js, or browser runtime.
- **Workspace shell:** `AppState -> Workspace -> DockArea -> TerminalPanel -> TerminalView`
- **Docking model:** `gpui-component` dock/tree primitives drive tabbed terminals, pane splits, and zoom state.
- **Terminal runtime:** `portable-pty` + `alacritty_terminal` + `gpui-terminal`
- **Persistence:** workspaces now restore their dock layout and terminal launch directories across restarts.
- **IPC:** Unix domain sockets on Unix by default, loopback TCP on non-Unix. Override with `RUMUX_SOCKET_PATH` or `RUMUX_SOCKET_ADDR`.

### Features

- Multiple workspaces with a vertical sidebar
- Docked terminal tabs and split panes
- Session save and restore for workspace layout
- Command palette and notification panel overlays
- Ghostty-compatible terminal font configuration
- Local RPC endpoint for automation and integration

### Building the Desktop App

Prerequisites: Rust plus the native libraries required by GPUI/wgpu on your platform.

```bash
# Debug
cargo run -p rumux-app

# Release
cargo build -p rumux-app --release
```

The binary is written to `target/release/rumux-app`.

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

## License

MIT
