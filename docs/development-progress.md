# Cross-platform completion evidence

The October 11, 2026 completion goal remains active. The target is the core cmux
workflow on macOS, Linux and Windows, including browser and remote surfaces.
See [ROADMAP.md](../ROADMAP.md) for outstanding acceptance criteria.

## Terminal foundation checkpoint

Starting commit: `a342a59`. Working branch: `codex/complete-cross-platform-rumux`.
Architectural reference: [herdr](https://github.com/herdrdev/herdr), inspected at
`75f46fe812dbc2946f7e4104c7c89b70f5a84f0a`. No reference code was copied.

This batch connects scrollback, selection and search; handles fragmented OSC
notifications, terminal titles and directory reports; replies to terminal
queries; reaps PTY children; targets notification navigation at its originating
pane; serves concurrent bounded RPC clients; and makes session saves atomic,
with backup recovery and protection against overwriting future schemas.

Validation on October 11, 2026:

- macOS ARM64, Rust 1.97.0, GPUI runtime shaders: 116 unit/integration tests
  passed, plus 28 doctests (28 existing illustrative doctests ignored).
- Real macOS desktop smoke: all ten checks passed in 3.32 seconds.
- Linux ARM64 in Debian Bookworm, Rust 1.97.0: full workspace tests passed,
  desktop and CLI built, and all ten real desktop smoke checks passed in
  0.95 seconds under Xvfb and a session D-Bus.
- Linux source archive SHA-256:
  `6d75ab415028510ac86885754af0130cbaf7267b357d40e7b6cbe8fe6c3c1a8f`.
  Image manifest:
  `sha256:adc8caf47af9a6e5835afbeb234dc8aa9ffa1103b33267173b5ba5dbd67a4e5c`.
- Formatting, whitespace, Python syntax and shell syntax checks passed.
  Clippy completed with existing warnings; this is not a warning-free result.
- Native visual inspection verified two terminal splits and the search bar's
  match count and highlights on macOS.

The smoke script exercises actual PTYs, concurrent clients, hidden-pane OSC
notifications, exact notification focus, directory inheritance, MCP, workspace
removal, pane close, single-instance activation and layout/directory restart.
It uses isolated temporary configuration and runtime directories.

Windows, Linux x86-64 and macOS Intel have not been executed locally. The native
CI matrix is prepared but has not run. Compilation and smoke evidence above do
not establish packaging, detached process survival, browser or SSH acceptance.
At that checkpoint no branch, PR, release or third-party comment had been published.

## Workspace and automation checkpoint

The second batch adds stable persisted UUIDs to workspaces and terminal panes,
pins asynchronous RPC calls to their original target, gives duplicated panes
independent identities, and exposes explicit agent activity through RPC, MCP,
OSC 133 and the sidebar. Terminals export their own surface ID for CLI calls.
The worktree CLI supports a selected agent executable, direct arguments and
`--no-agent`, and propagates agent failures.

Loopback TCP now requires a random per-profile token, rotated at startup. Unix
token files are private (0600). The CLI and MCP bridge authenticate without
exposing tokens in output. Direct clients must adopt the authentication field.

Validation on macOS ARM64: 120 unit/integration tests plus 28 doctests passed;
13 real desktop smoke checks passed in 3.79 seconds over Unix sockets and 14
checks passed in 2.20 seconds over authenticated TCP. Linux ARM64 passed the
same full test barrier and both smoke suites, in 1.24 and 1.25 seconds.
The Linux source archive for this run has SHA-256
`11ee4baa86965d3f8a436b968255f1aae0a5fd591a3f8e48851d3d7fdd6d142c`.
The subsequent sidebar subscription correction was visually verified on macOS;
the final local barrier and smoke runs also include that correction. A final
focused CLI regression check rejects non-object RPC params before injecting a
terminal's origin ID, avoiding a malformed-input panic (121 tests total).

## Published checkpoint and native CI

With explicit user approval, commits `5804afa` and `c583941` were pushed to the
public `LukasGuptaLeary/rumux` repository and draft
[PR #9](https://github.com/LukasGuptaLeary/rumux/pull/9) was opened.
[CI run 38165297268](https://github.com/LukasGuptaLeary/rumux/actions/runs/38165297268)
is bound to `c5839417e9200c509fbb31b68db6faf3083c0266`.
Rustfmt, CLI/core, Linux x86-64 desktop tests/smokes, macOS desktop tests/smokes,
and Linux packaging passed. Windows compiled and passed workspace Rust tests,
then failed to observe an OSC 133 status signal in the real desktop smoke.
The browser/platform batch below changes RPC text submission to the same CR as
the Enter key and explicitly enables VT output in Windows fixture producers.
These corrections require Windows revalidation; compilation alone is not a
successful Windows acceptance result.

## Browser and platform batch

Browser panels host the system engine through Wry: WKWebView, WebKitGTK or
WebView2. They share the dock's tabs/splits and stable identities, restore URLs,
track page titles, offer back/forward/reload/address controls, and hide native
views for inactive tabs/workspaces, overlays and other zoomed panes. CLI RPC and
named MCP tools support navigation, state, bounded snapshots, synchronous
JavaScript, CSS-selector clicks and form input. Page IPC reports focus only.

The initial Linux attempts exposed two concrete integration defects: creating a
WebKit context before GTK initialization, then GPUI 0.2.2's unimplemented X11 raw
window handles. A subsequent attempt exposed Wry's requirement for an Xlib XID
rather than an XCB handle. Initialization is now lazy, and an adapter borrows the
same live X11 window ID. `crates/gpui` retains the published 0.2.2 source and
Apache-2.0 license with only two source files changed: native X11 handles and a
fallible explicit X11 constructor. Rumux selects X11/XWayland when available
without changing the environment inherited by terminal programs. See
`crates/gpui/RUMUX-PATCHES.md` for provenance and the patch removal boundary.

Final local validation on October 11, 2026:

- macOS ARM64 and Linux ARM64: 123 workspace tests plus 28 doctests passed on
  each platform (28 existing illustrative doctests ignored).
- macOS native browser/PTY/CLI/MCP integration: 19 Unix-socket checks in 4.76 s
  and 20 authenticated-TCP checks in 3.63 s.
- Linux native browser/PTY/CLI/MCP integration: 19 checks in 3.30 s and 20 TCP
  checks in 3.15 s, with the normal WebKit sandbox. The XWayland selection case
  also passed all 19 checks in 3.15 s with a Wayland session variable present.
- Linux source archive SHA-256:
  `ee2d00c4e1a469797a07d722e02154a6d4c55b95df4d1dd0437faa90a88c0b0f`.
  Browser validation image manifest:
  `sha256:0f20c67a76a6da29474bcba1c914164996d2464fea2089da930d9f3e267ee987`.
- Native macOS inspection confirmed terminal/browser bounds, real input and
  button clicks, command-palette hiding, and browser zoom.
- Formatting, whitespace, Python/shell syntax checks passed. Clippy completed
  with existing warnings; the vendored GPUI source also has upstream float
  inference warnings on Rust 1.97.

The smoke suite additionally proves URL/ID restart restore, mixed pane routing,
form values containing quotes/Unicode, evaluation errors, rejected unsupported
navigation, Promise rejection and password-field omission in DOM snapshots.
The CI matrix now includes browser tests on all three platforms. Browser
behavior on Windows and the proposed Enter-key correction remain unqualified
until this batch runs there. Native Wayland browser hosting is unsupported;
Linux browser panes use X11/XWayland. Browser navigation history is not restored,
and JavaScript evaluation is synchronous.

## Browser CI correction

The approved browser commit `3641c49cf610a86191ea0dff2c9eada1777a3fe3`
was published to draft PR #9. In
[CI run 38168310346](https://github.com/LukasGuptaLeary/rumux/actions/runs/38168310346),
Rustfmt, CLI/core, Linux x86-64 desktop/browser tests and packaging, and macOS
desktop/browser tests passed. Windows passed Rust tests/build and advanced
through the terminal checks, then crashed during `browser.create`.
The stack identifies WebView2's constructor message pump running a pending GPUI
terminal task while the application was mutably borrowed.

The correction runs Windows browser construction on a foreground task outside
GPUI application/entity updates. A narrow GPUI native-parent lease delays HWND
destruction if the window closes during that constructor, then releases the
parent once construction finishes. Local macOS regression checks pass; Windows
runtime acceptance still requires the follow-up native run. The user explicitly
authorized publication of locally checked CI fixes on this branch.

## SSH workspace checkpoint

Typed OpenSSH workspaces are available through the Connect SSH palette dialog,
`rumux ssh` CLI, `workspace.create` RPC and `create_ssh_workspace` MCP tool.
Arguments keep host/config/key paths separate from the quoted remote directory.
Host authentication uses OpenSSH; test connections use strict known-host
verification. SSH panes export their stable surface ID and retain remote OSC 7
directories for new tabs and splits. Local forwarding binds loopback and belongs
to its original pane. Copies and new panes do not duplicate listeners.
Schema 5 persists connection parameters and launch directories for reconnect.

macOS ARM64 and Linux ARM64 passed 129 workspace tests and 28 doctests (28
existing illustrative doctests ignored). The extended native suite passed 25
Unix-socket checks and 26 authenticated-TCP checks on each platform: macOS
8.05 s / 7.01 s, Linux 6.13 s / 5.39 s. A repeat including the Windows browser
correction passed Linux checks in 5.85 s / 5.57 s. The Linux test source archive
has SHA-256 `b3801e8153055b721a124f9b0fd9b739acd021d2ad0eba445c519a731f458de3`;
validation image ID is
`sha256:1ff5dbac0e7e0489f48a914ab063717b4b9a900cd39b683525d9e97d54f5d474`.
Clippy completed with existing warnings.

Live macOS inspection verified invalid-destination errors and a real connection
through the advanced form, followed by remote output and a launch directory
containing spaces and a quote. Fixtures use their own loopback sshd, host/client
keys, config and known-hosts file. Linux StrictModes rejected a disposable
authorized-key file beneath world-writable `/tmp`; the fixture now places only
that temporary file under the test account's home, outside `.ssh`, and removes
it on cleanup. Password authentication remains disabled. A separate discovered
RPC bug (commands producing no final newline timed out despite completing) has
a regression test and now recognizes the numeric exit-marker suffix.

Windows SSH runtime remains unqualified. Remote shells must accept POSIX command
syntax. Restart currently establishes new SSH connections and shells; detached
process survival remains a separate acceptance boundary. No actual remote user
host, user SSH keys or known-hosts files were changed by these tests.

## Next acceptance boundaries

The user approved SSH commit `b7913f1` and the remaining locally checked
completion work on the existing public branch and draft PR #9. In
[CI run 38170021000](https://github.com/LukasGuptaLeary/rumux/actions/runs/38170021000),
the Windows constructor correction `e6de563` passed browser creation, DOM,
evaluation, form actions and MCP checks. Closing a terminal in a mixed layout
then overflowed the main-thread stack. Linux/macOS, formatting, CLI/core and
Linux packaging passed. Windows browser acceptance remains open.

The follow-up caches Windows native bounds and visibility transitions before
calling APIs that can synchronously deliver window messages. It does not raise
the stack limit or remove mixed-layout checks. A first cross-platform version
failed Linux's workspace-visibility check; the correction is now Windows-only.
The locally qualified snapshot contains the committed SSH batch and this
browser correction, without the unfinished terminal-service work. macOS ARM64
and Linux ARM64 each passed 129 workspace tests and 28 doctests, plus the full
25-check Unix-socket and 26-check authenticated-TCP browser/SSH suites.

Durable terminal ownership, Windows browser qualification, SSH workflows and three-platform installation proof
remain required. Any product claim must distinguish configured gates from
executed results, and restored layout from a still-running terminal process.

## Durable terminal ownership checkpoint

The terminal service now owns PTYs and the authoritative Alacritty parser
independently of the desktop. Stable pane UUIDs reattach to existing processes;
pane/workspace removal explicitly closes them off the GUI thread. CLI commands
`session status` and `session stop` expose the lifecycle. Parsed, validated cell
snapshots and history deltas preserve colors, fragmented escapes and active
alternate screens. Query replies continue while the GUI is absent. The service
retains title/CWD/status and acknowledges delivered pane notifications.

Private, atomic version 1 checkpoints retain bounded primary-screen history.
Cold restart launches fresh processes with that history and fresh input modes.
`persist_scrollback = false` opts newly started panes out of disk storage.
Unsupported and corrupt files are rejected and preserved, as are incompatible
live service protocols. Transport limits, grid limits, private endpoint
authentication, process locking and Windows ACL handling are documented in
`terminal-service-design.md`.

macOS ARM64 and Linux ARM64 passed 135 workspace tests and 28 doctests (28
existing illustrative doctests ignored). Both passed the full browser/SSH and
service suites over Unix sockets and authenticated TCP, now 33/34 checks.
They prove the same local shell PID, service PID, SSH client PID, remote shell
PID and forwarded listener across desktop restart; a detached background job;
exactly-once delivery of a retained notification; input after reattach;
alternate-screen/fragmented-CSI state; colored history after cold restart;
unsupported/corrupt history preservation; disabled disk persistence and
explicit close cleanup. A delayed-frame regression exercises the macOS
accepted-socket nonblocking inheritance correction. Windows fixture programs
encode multiline Python within one command argument for cmd.exe.

The local Alacritty 0.25.1 patch adds three narrow grid/render accessors and
retains all upstream source, Apache-2.0 license, provenance and reference
fixtures. Its separate upstream suite passed 132 unit tests, 45 reference tests
and one doctest. The Linux final code snapshot has SHA-256
`44c946780c1ab1ea2dc39c2ccef180170ae0840485504f2ba6dc32aee11910ea`;
image ID remains
`sha256:1ff5dbac0e7e0489f48a914ab063717b4b9a900cd39b683525d9e97d54f5d474`.
Formatting, whitespace, Python compilation and Clippy checks completed; existing
workspace/upstream warnings remain.

In [CI run 38171519402](https://github.com/LukasGuptaLeary/rumux/actions/runs/38171519402)
at `9ebc961`, Linux/macOS, CLI/core, formatting and Linux packaging passed.
Windows still overflowed the main-thread stack when closing a terminal in a
mixed browser layout. Bounds/visibility caching did not resolve that defect.
The service batch moves PTY shutdown off the GUI and adds opt-in debug window
message-depth diagnostics without raising the stack limit or weakening the
test. Its Windows acceptance, Windows SSH and three-platform installation
qualification remain open until native execution supplies evidence.

## Native installation checkpoint

The desktop archive now includes the CLI, native app/icon metadata, installers,
licenses, source commit/dirty-state metadata and per-file/archive checksums.
macOS installs a bundle and CLI links; Linux installs its binaries, icons and
desktop entry with an absolute quoted executable; Windows installs per user
with optional PATH and Start-menu integration. Installers reject changed files
before creating an installation and leave profile data outside it.

Local macOS ARM64 and Linux ARM64 release archives were extracted and installed
into owned directories containing spaces. The installed CLI and desktop then
passed 33 Unix-socket and 34 authenticated-TCP browser/SSH/service checks.
macOS took 9.57/8.99 s; Linux took 8.35/8.02 s. The macOS archive SHA-256 is
`5c6e9a11701c61fb1f711c85c7bdd936ed4f2020a68665441d2cd31f48802536`;
Linux is `0b5f9353bbe64d5118050d67d0d793a490726a148fb4988b17818fb5ef869507`.
They record base commit `7895852` plus an explicitly dirty packaging tree;
Linux also records frozen source archive SHA-256
`07b8cbcc609bc799f70c705282722948b8d8b2a515aaa91d6e9ebba5fe294159`.
The cold Linux optimized build took 4 min 51 s; the repeat reused the same Cargo
target volume. Older Python 3.11 lacks tarfile's extraction filter; the package
test now validates ordinary-file/directory members before extracting. Shell
syntax, Python compilation, actionlint and whitespace checks passed.

CI now qualifies installed optimized packages independently of debug builds on
Linux x86-64, macOS ARM64 and Windows x86-64. The desktop release workflow also
includes Windows and requires native install checks before upload. No release
workflow was dispatched; no tag or release was published. The existing
`v0.1.0` release contains CLI archives for all three platforms.

In [CI run 38174606832](https://github.com/LukasGuptaLeary/rumux/actions/runs/38174606832)
at `7895852`, Linux desktop/service/SSH passed. Windows passed service lifecycle
checks but still overflowed its stack closing a terminal in a mixed browser
layout; moving PTY shutdown off the GUI did not resolve it. macOS passed service
checks but an inherited SSH pane's `pwd` completion marker disappeared although
its output and prompt were visible. Forty local optimized SSH split/resize
repetitions passed. Failure diagnostics now compare authoritative daemon cells
with the view, and an opt-in Windows GDB/PDB trace captures the close failure
without changing stack limits, terminal behavior or required tests. Windows ACL
checks explicitly verify that the owned directory, endpoint and history grant
access only to the current user.

Failed native jobs previously discarded dependency caches and repeatedly spent
about seven minutes rebuilding Windows before the same crash. Cache-on-failure
now preserves dependency artifacts while Cargo still validates source/dependency
hashes and every native test continues to run. Windows browser/SSH and hosted
installed-package acceptance remain open.

## Native harness corrections

At `063e33e`, both macOS/Linux debug suites and the Linux installed package
passed. The installed macOS TCP suite caught a fixture race: its background
counter can briefly be empty during `write_text`. The check now retains the
first complete positive observation and retries incomplete reads, while still
requiring progress after desktop exit and the final count of 20. The installed
macOS package passed both full suites locally with this correction.

Windows stopped before the browser-close diagnostic because Python inherited
PowerShell 7 module paths when launching Windows PowerShell 5.1. ACL inspection
and installer execution now remove `PSModulePath` only from the child environment,
as documented by Microsoft; all permission and damaged-package checks remain.
This setup fix still requires native Windows execution.

A live macOS UI test confirmed keyboard input, terminal find and truecolor output.
Reading the closed app through native UI automation then relaunched the owned
test bundle without its isolated environment. The extra app and service were
identified by exact binary path/PID and stopped; available default session files
were retained in a private recovery directory. Subsequent lifecycle tests use
the owned launch/stop harness and do not query a closed app through automation.

## Windows permission and remote-path corrections

At `cb09fae`, the hosted debug and installed suites passed on both macOS and
Linux. Windows ACL inspection reached the real files and rejected an additional
allowed identity. `icacls /grant:r` replaces only the named user's explicit
grants; other explicit grants survive. The protection operation now supplies a
complete, owner-only protected DACL through the native Windows API, without an
intermediate broader access list. A Windows regression seeds an explicit
Everyone grant and independently inspects its removal with Get-Acl. The exact
production helper cross-compiles locally for Windows MSVC; native execution is
still required. Browser tests now precede service tests, retaining every check
while allowing the close diagnostic to run independently of permission checks.

SSH protocol parsing now explicitly uses remote POSIX paths on every client
platform. This corrects Windows OSC 7 decoding without changing local drive/UNC
handling and has a platform-independent regression. Native monospace defaults
are Menlo, Consolas and DejaVu Sans Mono; unavailable configured fonts fall back
to those defaults. Cell width uses ASCII font metrics instead of a box-drawing
fallback. Live macOS inspection verified aligned columns and borders in a
canonical-path owned bundle, then stopped its exact app and service through the
harness without querying the closed app.

The batch passed 136 workspace tests and 28 doctests on macOS ARM64 and Linux
ARM64, plus installed native browser/SSH/service suites on both transports.
macOS took 10.05/9.72 s. Linux frozen source archive SHA-256 is
`1da724d379ef17b282df3ee69817e62a55aaa74f5c93a5d8f3f4845c870ce5e1`;
the same validation image and cached Cargo volume were reused. Both package
manifests record `cb09fae` plus the explicitly dirty correction tree. Native
Windows browser, permissions, history and SSH acceptance remain open.

At `aace837`, all hosted macOS/Linux gates passed, and the installed optimized
Windows package passed 28 native checks in 21.73 s, including mixed browser
layout close and service authentication, private ACLs, detach/reattach and cold
history. The explicit-Everyone ACL regression passed in the Windows unit suite.
The Windows debug build still overflowed during the mixed-layout fixture.
GDB split multiword command-line instructions and did not attach; it now reads
an instruction file and attaches before the split's first redraw. This is a
diagnostic correction, not a demonstrated fix for the debug crash.

Owned WSL1 fixture scripts prepare a checksum-pinned Canonical rootfs for a
POSIX remote SSH target on disposable Windows CI machines. They suppress default
service startup, bind only a configured loopback sshd, verify a real Windows
OpenSSH client with strict host keys, and unregister only their uniquely named
distribution. The native desktop, service and client run outside WSL. POSIX
remote Python commands now use the fixture's interpreter rather than the
Windows client's executable. The common harness passed macOS/Linux suites;
Windows execution and workflow integration of this fixture remain pending.

## Rendering stack boundary and independent completion receipts

At `9cfb02a`, both native debug and installed suites passed on macOS/Linux;
Windows installed checks passed again. The Windows debugger finally attached
and captured the main-thread failure in `TerminalPanel::title_suffix` while
allocating a button during dock layout. It did not establish COM reentry as the
cause. Local debug prologues reserve approximately 118 KiB for that suffix and
129 KiB for the surrounding dock title bar, exceeding the recursive div's
128 KiB stack margin before another div check can run. GPUI now checks at view
and one-shot component rendering boundaries, growing existing same-thread
stack segments when less than 512 KiB remains. Main-thread limits and native
debug coverage are unchanged. The exact helper has a regression that nests
256 KiB frames on a 192 KiB thread; it passed locally on macOS/Linux.

A real macOS PTY command that emitted CSI 2J/3J and exited 7 erased its START
capture marker. RPC incorrectly timed out despite the visible exit status.
Command wrappers now emit bounded nonce-tagged OSC receipts, retained separately
by the authoritative service. Erasure no longer hides completion; incomplete
output capture is explicitly marked truncated. Parser tests cover every escape
fragment boundary and invalid receipts. Native Windows wrappers use a small
console mode of the installed app, with its path supplied by the service. The
exact Windows helper type-checks for MSVC locally; execution remains required.

The final local batch passed 139 workspace tests and 28 doctests on macOS ARM64
and Linux ARM64. Installed macOS suites passed 36/37 checks in 10.41/10.04 s;
the archive SHA-256 is
`7b63b0454aeef0e56bf2296e5c8490fe90cad57445aeee849738278a71dfb82f`.
Linux uses the same retained validation image/target cache and frozen source
archive SHA-256
`92db2841e4e226b8941af0526159e7b1c04fe0c40bc436319ba26997b530fef5`.
Installed Linux suites passed 36/37 checks in 8.24/8.10 s; its archive SHA-256
is `81065afe122237cf3e7fa0f950e97407fb4478d4dbae99ae6daea5e9b673144a`.
These packages record base `9cfb02a` plus the explicitly dirty completion tree.
The suite verifies remote screen/history erasure, native local receipts and
retention of only the latest 16 receipts. Windows cls clears the viewport;
its receipt is independently checked rather than assuming it erases history.

Windows CI now prepares the owned WSL1 target, proves a strict-host-key native
OpenSSH connection, and runs the complete debug and installed SSH suites.
Scripts pipe UTF-8 bytes to preserve POSIX LF. Verified rootfs bytes are cached
even if a later native check fails, and an always-run cleanup unregisters only
the owned distribution. This workflow is configured, not yet qualified evidence.

At `5d0a1b0`, all hosted macOS/Linux debug and installed gates passed.
Windows passed 140 workspace tests and 28 doctests, including the small-stack
render regression and owner-only ACL regression. Both Windows jobs prepared the
WSL1 target and their real Windows OpenSSH connection passed. Fixture teardown
then failed with WinError 32 on its Windows `sshd.log`: a WSL process retained
an inherited file handle. This prevented the desktop SSH stages from running;
it is not evidence that the live Windows rendering fix passed.

WSL server logs now remain inside the owned POSIX root, with Windows stdio sent
to the null device. Failure diagnostics read that log before removing the root.
No cleanup errors are ignored. The desktop gate runs when target setup succeeds,
even if the separate fixture verification fails, so a setup failure cannot hide
independent product diagnostics; the failed verification still fails the job.
The common harness passed installed macOS 36/37 checks in 10.58/9.99 s and
Linux's retained optimized binaries passed 36/37 checks in 8.78/8.06 s. No Rust
code changed in this fixture correction. Python compilation, actionlint and
whitespace checks passed; native Windows cleanup still requires execution.
