use gpui::*;

use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::list::ListItem;
use gpui_component::{ActiveTheme, Sizable};

use crate::root_view::*;

struct PaletteCommand {
    label: &'static str,
    shortcut: &'static str,
    mac_shortcut: &'static str,
    action: fn() -> Box<dyn Action>,
}

const COMMANDS: &[PaletteCommand] = &[
    PaletteCommand {
        label: "New Workspace",
        shortcut: "Ctrl+Shift+N",
        mac_shortcut: "Cmd+Shift+N",
        action: || Box::new(NewWorkspace),
    },
    PaletteCommand {
        label: "Close Workspace",
        shortcut: "Ctrl+Shift+W",
        mac_shortcut: "Cmd+Shift+W",
        action: || Box::new(CloseWorkspace),
    },
    PaletteCommand {
        label: "Split Right",
        shortcut: "Ctrl+Shift+D",
        mac_shortcut: "Cmd+D",
        action: || Box::new(SplitRight),
    },
    PaletteCommand {
        label: "Split Down",
        shortcut: "Ctrl+Alt+D",
        mac_shortcut: "Cmd+Shift+D",
        action: || Box::new(SplitDown),
    },
    PaletteCommand {
        label: "New Terminal",
        shortcut: "Ctrl+Shift+T",
        mac_shortcut: "Cmd+T",
        action: || Box::new(NewTerminal),
    },
    PaletteCommand {
        label: "Close Terminal",
        shortcut: "Ctrl+Shift+X",
        mac_shortcut: "Cmd+W",
        action: || Box::new(CloseTerminal),
    },
    PaletteCommand {
        label: "Next Workspace",
        shortcut: "Ctrl+Tab",
        mac_shortcut: "Ctrl+Tab",
        action: || Box::new(NextWorkspace),
    },
    PaletteCommand {
        label: "Previous Workspace",
        shortcut: "Ctrl+Shift+Tab",
        mac_shortcut: "Ctrl+Shift+Tab",
        action: || Box::new(PrevWorkspace),
    },
    PaletteCommand {
        label: "Duplicate Workspace",
        shortcut: "Ctrl+Alt+C",
        mac_shortcut: "Cmd+Shift+C",
        action: || Box::new(DuplicateWorkspace),
    },
    PaletteCommand {
        label: "Toggle Sidebar",
        shortcut: "Ctrl+B",
        mac_shortcut: "Cmd+B",
        action: || Box::new(ToggleSidebar),
    },
    PaletteCommand {
        label: "Toggle Pane Zoom",
        shortcut: "Ctrl+Shift+Enter",
        mac_shortcut: "Cmd+Shift+Enter",
        action: || Box::new(TogglePaneZoom),
    },
    PaletteCommand {
        label: "Find in Terminal",
        shortcut: "Ctrl+F",
        mac_shortcut: "Cmd+F",
        action: || Box::new(ToggleFindBar),
    },
    PaletteCommand {
        label: "Notifications",
        shortcut: "Ctrl+Shift+I",
        mac_shortcut: "Cmd+Shift+I",
        action: || Box::new(ToggleNotificationPanel),
    },
    PaletteCommand {
        label: "Jump to Unread",
        shortcut: "Ctrl+Shift+U",
        mac_shortcut: "Cmd+Shift+U",
        action: || Box::new(JumpToUnread),
    },
    PaletteCommand {
        label: "Quit",
        shortcut: "Ctrl+Q",
        mac_shortcut: "Cmd+Q",
        action: || Box::new(QuitApp),
    },
];

pub struct CommandPalette {
    input: Entity<InputState>,
    query: String,
    selected: usize,
    filtered: Vec<usize>,
    pending_action: Option<fn() -> Box<dyn Action>>,
    pub focus_handle: FocusHandle,
    _input_sub: gpui::Subscription,
}

impl CommandPalette {
    fn display_shortcut(cmd: &PaletteCommand) -> &'static str {
        if cfg!(target_os = "macos") {
            cmd.mac_shortcut
        } else {
            cmd.shortcut
        }
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("Type a command...", window, cx);
            state
        });

        input.update(cx, |state, cx| {
            state.focus(window, cx);
        });

        let input_sub = cx.subscribe(
            &input,
            |palette: &mut Self, editor, event: &InputEvent, cx| {
                match event {
                    InputEvent::Change => {
                        let text = editor.read(cx).text().to_string();
                        palette.query = text;
                        palette.selected = 0;
                        palette.update_filtered();
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => {
                        // Store pending action, will dispatch in render with window access
                        if let Some(&cmd_idx) = palette.filtered.get(palette.selected) {
                            palette.pending_action = Some(COMMANDS[cmd_idx].action);
                            cx.emit(PaletteEvent::Dismiss);
                            cx.notify();
                        }
                    }
                    _ => {}
                }
            },
        );

        let filtered = (0..COMMANDS.len()).collect();

        Self {
            input,
            query: String::new(),
            selected: 0,
            filtered,
            pending_action: None,
            focus_handle,
            _input_sub: input_sub,
        }
    }

    fn update_filtered(&mut self) {
        if self.query.is_empty() {
            self.filtered = (0..COMMANDS.len()).collect();
        } else {
            let q = self.query.to_lowercase();
            self.filtered = COMMANDS
                .iter()
                .enumerate()
                .filter(|(_, cmd)| cmd.label.to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .collect();
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "escape" => {
                cx.emit(PaletteEvent::Dismiss);
            }
            "up" => {
                if self.selected > 0 {
                    self.selected -= 1;
                    cx.notify();
                }
            }
            "down" => {
                if self.selected + 1 < self.filtered.len() {
                    self.selected += 1;
                    cx.notify();
                }
            }
            _ => {}
        }
    }
}

pub enum PaletteEvent {
    Dismiss,
}

impl EventEmitter<PaletteEvent> for CommandPalette {}

impl Render for CommandPalette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Dispatch pending action from previous frame
        if let Some(action_fn) = self.pending_action.take() {
            let action = action_fn();
            window.dispatch_action(action, cx);
        }

        let mut list = div().flex_1().overflow_hidden();
        for (display_idx, &cmd_idx) in self.filtered.iter().enumerate() {
            let cmd = &COMMANDS[cmd_idx];
            let is_selected = display_idx == self.selected;
            let shortcut = Self::display_shortcut(cmd);

            let row = ListItem::new(display_idx)
                .selected(is_selected)
                .on_click(cx.listener(move |palette, _event, window, cx| {
                    palette.selected = display_idx;
                    if let Some(&cmd_idx) = palette.filtered.get(palette.selected) {
                        let action = (COMMANDS[cmd_idx].action)();
                        cx.emit(PaletteEvent::Dismiss);
                        window.dispatch_action(action, cx);
                    }
                }))
                .child(
                    div()
                        .w_full()
                        .flex()
                        .justify_between()
                        .items_center()
                        .child(div().text_size(px(13.0)).child(cmd.label))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(cx.theme().muted_foreground)
                                .px(px(6.0))
                                .py(px(1.0))
                                .bg(cx.theme().secondary)
                                .rounded(px(3.0))
                                .child(shortcut),
                        ),
                );

            list = list.child(row);
        }

        if self.filtered.is_empty() {
            list = list.child(
                div()
                    .p(px(16.0))
                    .text_color(cx.theme().muted_foreground)
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
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_palette, _event, _window, cx| {
                    cx.emit(PaletteEvent::Dismiss);
                }),
            )
            .child(
                div()
                    .w(px(480.0))
                    .max_h(px(400.0))
                    .bg(cx.theme().popover)
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(cx.theme().border)
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .on_mouse_down(MouseButton::Left, |_event, _window, cx| {
                        cx.stop_propagation();
                    })
                    // Input area
                    .child(
                        div()
                            .px(px(12.0))
                            .py(px(8.0))
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(Input::new(&self.input).appearance(false).small()),
                    )
                    // Command list
                    .child(list),
            )
    }
}
