use gpui::*;

use crate::root_view::*;
use crate::theme;

pub struct CommandPalette {
    query: String,
    selected: usize,
    commands: Vec<PaletteCommand>,
    pub focus_handle: FocusHandle,
}

struct PaletteCommand {
    label: &'static str,
    shortcut: &'static str,
    action: Box<dyn Action>,
}

impl CommandPalette {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            query: String::new(),
            selected: 0,
            commands: vec![
                PaletteCommand {
                    label: "New Workspace",
                    shortcut: "Ctrl+Shift+N",
                    action: Box::new(NewWorkspace),
                },
                PaletteCommand {
                    label: "Close Workspace",
                    shortcut: "Ctrl+Shift+W",
                    action: Box::new(CloseWorkspace),
                },
                PaletteCommand {
                    label: "Split Right",
                    shortcut: "Ctrl+Shift+D",
                    action: Box::new(SplitRight),
                },
                PaletteCommand {
                    label: "Split Down",
                    shortcut: "Ctrl+Alt+D",
                    action: Box::new(SplitDown),
                },
                PaletteCommand {
                    label: "New Terminal",
                    shortcut: "Ctrl+Shift+T",
                    action: Box::new(NewTerminal),
                },
                PaletteCommand {
                    label: "Close Terminal",
                    shortcut: "Ctrl+Shift+X",
                    action: Box::new(CloseTerminal),
                },
                PaletteCommand {
                    label: "Next Workspace",
                    shortcut: "Ctrl+Tab",
                    action: Box::new(NextWorkspace),
                },
                PaletteCommand {
                    label: "Previous Workspace",
                    shortcut: "Ctrl+Shift+Tab",
                    action: Box::new(PrevWorkspace),
                },
                PaletteCommand {
                    label: "Duplicate Workspace",
                    shortcut: "Ctrl+Shift+C",
                    action: Box::new(DuplicateWorkspace),
                },
                PaletteCommand {
                    label: "Toggle Sidebar",
                    shortcut: "Ctrl+B",
                    action: Box::new(ToggleSidebar),
                },
                PaletteCommand {
                    label: "Toggle Pane Zoom",
                    shortcut: "Ctrl+Shift+Enter",
                    action: Box::new(TogglePaneZoom),
                },
                PaletteCommand {
                    label: "Find in Terminal",
                    shortcut: "Ctrl+F",
                    action: Box::new(ToggleFindBar),
                },
                PaletteCommand {
                    label: "Notifications",
                    shortcut: "Ctrl+Shift+I",
                    action: Box::new(ToggleNotificationPanel),
                },
                PaletteCommand {
                    label: "Jump to Unread",
                    shortcut: "Ctrl+Shift+U",
                    action: Box::new(JumpToUnread),
                },
                PaletteCommand {
                    label: "Quit",
                    shortcut: "Ctrl+Q",
                    action: Box::new(QuitApp),
                },
            ],
            focus_handle: cx.focus_handle(),
        }
    }

    fn filtered_commands(&self) -> Vec<usize> {
        if self.query.is_empty() {
            return (0..self.commands.len()).collect();
        }
        let q = self.query.to_lowercase();
        self.commands
            .iter()
            .enumerate()
            .filter(|(_, cmd)| cmd.label.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect()
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "escape" => {
                window.dispatch_action(Box::new(ToggleCommandPalette), cx);
            }
            "up" => {
                if self.selected > 0 {
                    self.selected -= 1;
                    cx.notify();
                }
            }
            "down" => {
                let filtered = self.filtered_commands();
                if self.selected + 1 < filtered.len() {
                    self.selected += 1;
                    cx.notify();
                }
            }
            "enter" => {
                let filtered = self.filtered_commands();
                if let Some(&cmd_idx) = filtered.get(self.selected) {
                    let action = self.commands[cmd_idx].action.boxed_clone();
                    window.dispatch_action(Box::new(ToggleCommandPalette), cx);
                    window.dispatch_action(action, cx);
                }
            }
            "backspace" => {
                self.query.pop();
                self.selected = 0;
                cx.notify();
            }
            key => {
                if key.len() == 1 && !event.keystroke.modifiers.control {
                    self.query.push_str(key);
                    self.selected = 0;
                    cx.notify();
                }
            }
        }
    }
}

impl Render for CommandPalette {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let filtered = self.filtered_commands();

        let mut list = div().flex_1().overflow_hidden();
        for (display_idx, &cmd_idx) in filtered.iter().enumerate() {
            let cmd = &self.commands[cmd_idx];
            let is_selected = display_idx == self.selected;

            let mut row = div()
                .px(px(16.0))
                .py(px(8.0))
                .flex()
                .justify_between()
                .items_center()
                .cursor_pointer();

            if is_selected {
                row = row.bg(rgb(theme::BG_HOVER));
            }

            row = row
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(theme::TEXT_PRIMARY))
                        .child(cmd.label),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(theme::TEXT_DIM))
                        .px(px(6.0))
                        .py(px(1.0))
                        .bg(rgb(theme::BG_SURFACE))
                        .rounded(px(3.0))
                        .child(cmd.shortcut),
                );

            list = list.child(row);
        }

        if filtered.is_empty() {
            list = list.child(
                div()
                    .p(px(16.0))
                    .text_align(TextAlign::Center)
                    .text_color(rgb(theme::TEXT_DIM))
                    .child("No matching commands"),
            );
        }

        // Overlay
        div()
            .absolute()
            .inset_0()
            .bg(Hsla {
                h: 0.0,
                s: 0.0,
                l: 0.0,
                a: 0.5,
            })
            .flex()
            .justify_center()
            .pt(px(100.0))
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .child(
                div()
                    .w(px(480.0))
                    .max_h(px(400.0))
                    .bg(rgb(theme::BG_SECONDARY))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(rgb(theme::BORDER))
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    // Input area
                    .child(
                        div()
                            .px(px(16.0))
                            .py(px(12.0))
                            .border_b_1()
                            .border_color(rgb(theme::BORDER))
                            .child(if self.query.is_empty() {
                                div()
                                    .text_size(px(14.0))
                                    .text_color(rgb(theme::TEXT_DIM))
                                    .child("Type a command...")
                            } else {
                                div()
                                    .text_size(px(14.0))
                                    .text_color(rgb(theme::TEXT_PRIMARY))
                                    .child(self.query.clone())
                            }),
                    )
                    // Command list
                    .child(list),
            )
    }
}
