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

CLI release artifacts are built and uploaded by GitHub Actions in `.github/workflows/release-cli.yml`.

Supported release assets today:

- `rumux-x86_64-unknown-linux-gnu.tar.gz`
- `rumux-x86_64-apple-darwin.tar.gz`
- `rumux-aarch64-apple-darwin.tar.gz`
- `rumux-x86_64-pc-windows-msvc.zip`
- `rumux-checksums.txt`

The installer script at [install.sh](install.sh) consumes those assets directly from GitHub Releases.

Desktop app artifacts are still a separate manual packaging task.

## 4. Tag and Publish

- Create a git tag like `v0.1.0`
- Push the tag
- Let the `Release CLI` workflow attach CLI binaries and checksums to the draft release
- Review release notes, then publish the draft release

If you need to rebuild assets for an existing draft tag, run the workflow manually:

```bash
gh workflow run release-cli.yml -f tag=v0.1.0
```

## 5. Optional Crates.io Publish

The CLI and shared core crate can be published separately if crate metadata, ownership, and repository settings are ready.

Typical order:

```bash
cargo publish -p rumux-core
cargo publish -p rumux-cli
```

`rumux-app` is intentionally marked `publish = false`.
