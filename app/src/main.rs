#![warn(clippy::all)]

use gpui::*;
use gpui_terminal::{ColorPalette, TerminalConfig, TerminalView};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::sync::Arc;

struct TerminalApp {
    terminal: Entity<TerminalView>,
}

impl TerminalApp {
    fn new(terminal: Entity<TerminalView>) -> Self {
        Self { terminal }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;

        if keystroke.modifiers.control && (keystroke.key == "+" || keystroke.key == "=") {
            self.terminal.update(cx, |terminal, cx| {
                let mut config = terminal.config().clone();
                config.font_size += px(1.0);
                terminal.update_config(config, cx);
            });
            cx.stop_propagation();
        } else if keystroke.modifiers.control && keystroke.key == "-" {
            self.terminal.update(cx, |terminal, cx| {
                let mut config = terminal.config().clone();
                if config.font_size > px(6.0) {
                    config.font_size -= px(1.0);
                    terminal.update_config(config, cx);
                }
            });
            cx.stop_propagation();
        }
    }
}

impl Render for TerminalApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .on_key_down(cx.listener(Self::on_key_down))
            .child(self.terminal.clone())
    }
}

fn main() {
    let app = Application::new();

    app.run(move |cx| {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("Failed to open PTY");

        let mut cmd = CommandBuilder::new(&shell);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "rumux");

        let _child = pair
            .slave
            .spawn_command(cmd)
            .expect("Failed to spawn shell");

        let writer = pair.master.take_writer().expect("Failed to get PTY writer");
        let reader = pair
            .master
            .try_clone_reader()
            .expect("Failed to get PTY reader");

        let pty_master = Arc::new(parking_lot::Mutex::new(pair.master));
        drop(pair.slave);

        let pty_master_clone = pty_master.clone();
        cx.spawn(async move |cx| {
            let colors = ColorPalette::builder()
                .background(0x1e, 0x1e, 0x2e)
                .foreground(0xcd, 0xd6, 0xf4)
                .cursor(0xf5, 0xe0, 0xdc)
                .black(0x45, 0x47, 0x5a)
                .red(0xf3, 0x8b, 0xa8)
                .green(0xa6, 0xe3, 0xa1)
                .yellow(0xf9, 0xe2, 0xaf)
                .blue(0x89, 0xb4, 0xfa)
                .magenta(0xf5, 0xc2, 0xe7)
                .cyan(0x94, 0xe2, 0xd5)
                .white(0xba, 0xc2, 0xde)
                .bright_black(0x58, 0x5b, 0x70)
                .bright_red(0xf3, 0x8b, 0xa8)
                .bright_green(0xa6, 0xe3, 0xa1)
                .bright_yellow(0xf9, 0xe2, 0xaf)
                .bright_blue(0x89, 0xb4, 0xfa)
                .bright_magenta(0xf5, 0xc2, 0xe7)
                .bright_cyan(0x94, 0xe2, 0xd5)
                .bright_white(0xa6, 0xad, 0xc8)
                .build();

            let config = TerminalConfig {
                font_family: "JetBrains Mono".into(),
                font_size: px(14.0),
                cols: 80,
                rows: 24,
                scrollback: 10_000,
                line_height_multiplier: 1.0,
                padding: Edges::all(px(8.0)),
                colors,
            };

            let pty_for_resize = pty_master_clone.clone();
            let resize_callback = move |cols: usize, rows: usize| {
                let _ = pty_for_resize.lock().resize(PtySize {
                    cols: cols as u16,
                    rows: rows as u16,
                    pixel_width: 0,
                    pixel_height: 0,
                });
            };

            cx.open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("rumux".into()),
                        ..Default::default()
                    }),
                    window_min_size: Some(size(px(400.0), px(300.0))),
                    ..Default::default()
                },
                |window, cx| {
                    let terminal = cx.new(|cx| {
                        TerminalView::new(writer, reader, config, cx)
                            .with_resize_callback(resize_callback)
                            .with_exit_callback(|_window, cx| {
                                cx.quit();
                            })
                    });

                    terminal.read(cx).focus_handle().focus(window);

                    cx.new(|_cx| TerminalApp::new(terminal))
                },
            )?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
