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

rumux includes a cross-platform desktop terminal application built with Tauri v2, xterm.js, and React. It replicates the [cmux desktop app](https://cmux.com) with support for macOS, Linux, and Windows.

### Features

- Multiple workspaces with a vertical tab sidebar
- Split panes (horizontal and vertical) with drag-to-resize dividers
- Multiple terminal surfaces per pane (tabbed)
- xterm.js terminal with WebGL rendering, 256-color support, Unicode
- Embedded browser surfaces with URL bar and navigation
- Notification system (OSC 777/99/9 escape sequences, OS notifications)
- Unix socket API server (`/tmp/rumux.sock`) for programmatic control
- Command palette (Cmd+Shift+P) with fuzzy search
- Session save/restore (layout, directories, browser URLs)
- Ghostty-compatible configuration
- Custom commands via `rumux.json`/`cmux.json`

### Building the Desktop App

Prerequisites: Rust, Node.js, pnpm, and platform-specific dependencies.

**Linux:**
```bash
sudo apt-get install -y libgtk-3-dev libwebkit2gtk-4.1-dev libglib2.0-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev
```

**Build:**
```bash
cd app/src && pnpm install
cargo tauri build
```

The built binary is at `target/release/rumux-app`.

**Development:**
```bash
cargo tauri dev
```

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Cmd+N | New workspace |
| Cmd+Shift+W | Close workspace |
| Cmd+Shift+R | Rename workspace |
| Cmd+T | New terminal surface |
| Cmd+W | Close surface |
| Cmd+D | Split right |
| Cmd+Shift+D | Split down |
| Cmd+Shift+P | Command palette |
| Cmd+Shift+I | Notification panel |
| Cmd+F | Find in terminal |
| Cmd+K | Clear terminal |
| Cmd+, | Settings |

Use Ctrl instead of Cmd on Linux/Windows.

## License

MIT
