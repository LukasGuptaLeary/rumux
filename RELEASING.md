# Releasing

This file documents the current release checklist for `rumux`.

## 1. Validate the Tree

Run from a clean branch:

```bash
cargo fmt --all --check
cargo test -p rumux-core -p rumux-cli
cargo check -p rumux-app
```

## 2. Update Release Metadata

- Bump `[workspace.package].version` in `Cargo.toml`
- Update [CHANGELOG.md](CHANGELOG.md)
- Review [README.md](README.md) if commands, shortcuts, or install guidance changed

The workspace crates inherit their version from the root manifest.

## 3. Build Release Artifacts

```bash
cargo build -p rumux-cli --release
cargo build -p rumux-app --release
```

Expected outputs:

- `target/release/rumux`
- `target/release/rumux-app`

## 4. Tag and Publish

- Create a git tag like `v0.1.0`
- Draft release notes from the changelog and merged PRs
- Attach release binaries if you are distributing GitHub release artifacts

## 5. Optional Crates.io Publish

The CLI and shared core crate can be published separately if crate metadata, ownership, and repository settings are ready.

Typical order:

```bash
cargo publish -p rumux-core
cargo publish -p rumux-cli
```

`rumux-app` is intentionally marked `publish = false`.
