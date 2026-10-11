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

Use the registry-normalized `Cargo.toml` so all other dependencies keep their
published versions. `Cargo.toml.orig` remains upstream provenance. Remove this
patch when a qualified published GPUI version provides these handles.
