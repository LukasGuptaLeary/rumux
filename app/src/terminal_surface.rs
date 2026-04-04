use anyhow::Result;
use gpui::*;
use gpui_terminal::{TerminalConfig, TerminalView};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::path::Path;
use std::sync::Arc;

use crate::config::RumuxConfig;
use crate::theme;

pub fn spawn_terminal_view(
    cx: &mut App,
    cwd: Option<&Path>,
    on_exit: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
) -> Result<Entity<TerminalView>> {
    spawn_terminal_view_with_config(cx, cwd, on_exit, None, None)
}

pub fn spawn_terminal_view_with_config(
    cx: &mut App,
    cwd: Option<&Path>,
    on_exit: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    config_override: Option<&RumuxConfig>,
    output_callback: Option<Arc<dyn Fn(&[u8]) + Send + Sync>>,
) -> Result<Entity<TerminalView>> {
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let mut cmd = CommandBuilder::new(&shell);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("TERM_PROGRAM", "rumux");

    if let Some(dir) = cwd {
        cmd.cwd(dir);
    }

    let _child = pair.slave.spawn_command(cmd)?;

    let writer = pair.master.take_writer()?;
    let reader = pair.master.try_clone_reader()?;
    let pty_master = Arc::new(parking_lot::Mutex::new(pair.master));
    drop(pair.slave);

    let font_family = config_override
        .map(|c| c.font_family.clone())
        .unwrap_or_else(|| "JetBrains Mono".to_string());
    let font_size_val = config_override.map(|c| c.font_size).unwrap_or(14.0);
    let scrollback = config_override.map(|c| c.scrollback).unwrap_or(10_000);

    let config = TerminalConfig {
        font_family,
        font_size: px(font_size_val),
        cols: 80,
        rows: 24,
        scrollback,
        line_height_multiplier: 1.0,
        padding: Edges::all(px(4.0)),
        colors: theme::catppuccin_mocha(),
    };

    let pty_for_resize = pty_master.clone();
    let resize_callback = move |cols: usize, rows: usize| {
        let _ = pty_for_resize.lock().resize(PtySize {
            cols: cols as u16,
            rows: rows as u16,
            pixel_width: 0,
            pixel_height: 0,
        });
    };

    let terminal = cx.new(|cx| {
        let mut view = TerminalView::new(writer, reader, config, cx)
            .with_resize_callback(resize_callback);

        if let Some(exit_cb) = on_exit {
            view = view.with_exit_callback(move |window, cx| exit_cb(window, cx));
        }

        if let Some(output_cb) = output_callback {
            view = view.with_output_callback(move |bytes| output_cb(bytes));
        }

        view
    });

    Ok(terminal)
}
