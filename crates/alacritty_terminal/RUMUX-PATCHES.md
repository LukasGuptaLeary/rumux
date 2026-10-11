# Rumux Alacritty terminal patch

Source: crates.io `alacritty_terminal` 0.25.1, upstream commit
`12082407d9cb3384a2bd83c578e556ec3faa2c4d`. All source, Apache-2.0 license,
registry provenance and reference-test fixtures are retained.

Three additions support a separate PTY owner and a read-only desktop mirror:

- `Grid::history_serial` counts rows moved into history, including scrolling
  after the history buffer reaches capacity. It is excluded from existing
  serialized grid fixtures and does not change terminal emulation.
- `Term::apply_render_state` installs mode flags, changed colors and cursor
  style on a render mirror. The service retains the authoritative VTE parser;
  the desktop never parses output after applying this mirror state.
- `Term::primary_grid` reads primary-screen history while an alternate-screen
  application is active, without swapping buffers or replaying terminal effects.

This avoids cutting a bounded raw ANSI replay in the middle of an escape
sequence, UTF-8 character or alternate-screen application. Preserve the narrow
patch and run the upstream unit/reference tests when changing it. Remove this
patch when a qualified upstream API supplies the same state and history cursor.
