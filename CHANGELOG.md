# Changelog

All notable changes to this project should be recorded in this file.

## [Unreleased]

## [0.2.0] - 2026-10-11

### Added

- Native browser tabs/splits with navigation, DOM snapshots, JavaScript and form automation
- Typed SSH workspaces with remote directory inheritance and loopback forwarding
- A separate terminal service that preserves processes across desktop exit and reattaches by pane ID
- Private bounded terminal history, cold restore and explicit session status/stop commands
- Desktop and CLI packages with icons, per-user installers, metadata and tamper checks on all three platforms
- Native debug and installed-package checks for terminal, browser, SSH, CLI and MCP workflows

### Changed

- Terminal scrollback, selection, search, colors, protocol replies, notifications and agent status
- Stable workspace/pane identities and atomic versioned sessions with recovery
- Configurable worktree agents and preparation without launching an agent

### Fixed

- Windows WebView2 construction and finite debug toolbar stack overflow
- Windows private-state ACLs and remote POSIX directory parsing
- Command completion after screen/history erasure, with explicit incomplete output reporting

## [0.1.0] - 2026-04-05

### Added

- Open-source release docs, governance files, and CI
- Public GitHub repository, issue templates, and PR template
- GitHub Actions workflow for publishing CLI release artifacts and checksums
- `install.sh` for release-based CLI installation on supported macOS and Linux targets

### Changed

- Desktop shell promoted to the canonical `app/src/` implementation
- Cargo workspace metadata and package boundaries normalized for public release
- Public install and update instructions aligned with the release-based CLI workflow
