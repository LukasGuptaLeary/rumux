# Contributing

Thanks for working on `rumux`.

## Before You Start

- Read [README.md](README.md) for the project overview and current architecture.
- Check [SECURITY.md](SECURITY.md) before reporting anything that could be a security issue.
- Use the GitHub issue templates for bugs and feature requests.

## Development Setup

Requirements:

- Rust 1.88 or newer
- Git on `PATH`
- Native system libraries required by GPUI if you are building the desktop app

Common commands:

```bash
# Format
cargo fmt --all

# CLI and shared library
cargo test -p rumux-core -p rumux-cli

# Desktop app compile check
cargo check -p rumux-app

# Run the desktop app
cargo run -p rumux-app
```

The terminal component is part of the test workspace. For the complete local
barrier on macOS with Command Line Tools:

```bash
cargo fmt --all --check
cargo test --workspace --features rumux-app/runtime-shaders
cargo build -p rumux-app -p rumux-cli --features rumux-app/runtime-shaders
python3 scripts/smoke-desktop.py
```

The smoke test launches real PTYs and a desktop window in a disposable profile;
it verifies automation, notifications, concurrency, and restart restore.
It needs a graphics session. Linux can use `dbus-run-session -- xvfb-run -a`.

A local Linux build uses the same system prerequisites as CI:

```bash
docker build -f scripts/linux.Dockerfile -t rumux-linux-check .
docker run --rm -v "$PWD:/workspace:ro" -v rumux-linux-cargo:/usr/local/cargo \
  -v rumux-linux-target:/build -e CARGO_TARGET_DIR=/build rumux-linux-check \
  bash -c 'cargo test --workspace --locked && cargo build -p rumux-app -p rumux-cli --locked && dbus-run-session -- xvfb-run -a python3 scripts/smoke-desktop.py --app /build/debug/rumux-app --cli /build/debug/rumux'
```

On Linux, desktop app builds usually need the standard development headers for Wayland and/or X11, `xkbcommon`, `fontconfig`, and `dbus`.

## Workflow

1. Start from a clean branch.
2. Keep changes focused on one problem or feature.
3. Add or update tests when behavior changes.
4. Update docs when commands, shortcuts, or architecture change.
5. Run the relevant checks before opening a pull request.

## Architecture

The current desktop shell lives directly under `app/src/`. The legacy `v1` path has been removed.

- `crates/rumux-cli`: CLI entrypoint and command dispatch
- `crates/rumux-core`: shared config, git, runtime, and error code
- `app/src`: GPUI desktop application shell
- `crates/gpui-component`: vendored dock and UI primitives
- `crates/gpui-terminal`: vendored terminal component

For a deeper architecture map, see [CLAUDE.md](CLAUDE.md).

## Pull Requests

Before opening a pull request, make sure:

- `cargo fmt --all` is clean
- `cargo test -p rumux-core -p rumux-cli` passes
- `cargo check -p rumux-app` passes if your changes affect the desktop app
- The PR description explains the user-facing change and any follow-up work

## Licensing

By contributing to this repository, you agree that your contributions will be licensed under the same terms as the project code in [LICENSE](LICENSE).
