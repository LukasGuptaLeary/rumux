# Rumux GPUI patch

Source: crates.io `gpui` 0.2.2, upstream Zed commit
`69e2130295c2649963eb639fc70b4f2ee8ea1624` (registry provenance records a dirty
upstream publication). Preserve the included Apache-2.0 license.

Rumux changes `src/platform/linux/x11/window.rs`: retain the selected
visual/screen and implement `HasWindowHandle`/`HasDisplayHandle` for the live
X11 window. Upstream's methods panic. The handles borrow the window's lifetime,
and the window retains its XCB connection. This enables Wry child views without
window discovery, private structure casts, or changes to the rendering backend.

`src/app.rs` also adds a fallible `Application::x11()` constructor so Rumux can
use an XWayland parent on Wayland desktops without changing environment variables
inherited by shells. The original default constructor and Wayland path remain.

`src/window.rs`, `src/platform.rs`, and the Windows `window.rs`/`events.rs` add
a short native-parent lease. It defers HWND destruction while Rumux constructs
WebView2 outside a GPUI application borrow. WebView2's synchronous constructor
pumps messages, including pending terminal tasks; constructing it inside an
entity update caused a native CI crash (`RefCell already borrowed`). The lease
keeps the parent valid if the user closes the window during construction and
releases it when construction finishes. Other platforms retain their default
window lifecycle.

Debug builds additionally trace a deeply nested Windows message dispatch when
`RUMUX_TERMINAL_TRACE` is set. It records a backtrace without changing message
handling or the stack limit, to diagnose the mixed-pane close failure in native
CI. Release builds do not include this diagnostic.

`src/view.rs`, `src/element.rs` and `src/render_stack.rs` check available stack
before typed/erased views and one-shot components render.
The existing div layout checks cover recursion, but debug toolbar builders can
consume more than their default 128 KiB margin before reaching another div.
The Windows native trace stops in a terminal toolbar's element allocation;
local debug prologues reserve about 118 KiB for its suffix and 129 KiB for the
dock title bar. The rendering boundary uses stacker's existing same-thread
segments with a 512 KiB margin and 2 MiB allocation. It leaves the main-thread
stack limit unchanged. A small-stack regression nests large finite render
frames through this exact boundary. Keep the native debug layout check enabled.

Use the registry-normalized `Cargo.toml` so all other dependencies keep their
published versions. `Cargo.toml.orig` remains upstream provenance. Remove this
patch when a qualified published GPUI version provides these handles.
