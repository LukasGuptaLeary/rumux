use gpui::*;
use gpui_terminal::TerminalView;

use crate::terminal_surface::spawn_terminal_view;
use crate::theme;

pub struct Pane {
    terminals: Vec<Entity<TerminalView>>,
    active_idx: usize,
    focus_handle: FocusHandle,
}

impl Pane {
    pub fn new(terminal: Entity<TerminalView>, cx: &mut Context<Self>) -> Self {
        Self {
            terminals: vec![terminal],
            active_idx: 0,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn active_terminal(&self) -> &Entity<TerminalView> {
        &self.terminals[self.active_idx]
    }

    pub fn terminal_count(&self) -> usize {
        self.terminals.len()
    }

    pub fn add_terminal(&mut self, cx: &mut App) {
        if let Ok(term) = spawn_terminal_view(cx, None, None) {
            self.terminals.push(term);
            self.active_idx = self.terminals.len() - 1;
        }
    }

    pub fn close_active_terminal(&mut self) -> bool {
        if self.terminals.len() <= 1 {
            return true; // pane should be removed
        }
        self.terminals.remove(self.active_idx);
        if self.active_idx >= self.terminals.len() {
            self.active_idx = self.terminals.len() - 1;
        }
        false
    }

    pub fn activate_terminal(&mut self, idx: usize) {
        if idx < self.terminals.len() {
            self.active_idx = idx;
        }
    }

    pub fn next_terminal(&mut self) {
        if self.terminals.len() > 1 {
            self.active_idx = (self.active_idx + 1) % self.terminals.len();
        }
    }

    pub fn prev_terminal(&mut self) {
        if self.terminals.len() > 1 {
            self.active_idx = if self.active_idx == 0 {
                self.terminals.len() - 1
            } else {
                self.active_idx - 1
            };
        }
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }
}

impl Render for Pane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Focus the active terminal
        self.active_terminal()
            .read(cx)
            .focus_handle()
            .focus(window);

        let mut container = div().size_full().flex().flex_col().track_focus(&self.focus_handle);

        // Tab bar (only when multiple terminals)
        if self.terminals.len() > 1 {
            let mut tab_bar = div()
                .flex()
                .flex_row()
                .h(px(28.0))
                .bg(rgb(theme::BG_SECONDARY))
                .border_b_1()
                .border_color(rgb(theme::BORDER));

            for i in 0..self.terminals.len() {
                let is_active = i == self.active_idx;
                let mut tab = div()
                    .px(px(12.0))
                    .h_full()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .text_size(px(12.0))
                    .on_mouse_down(MouseButton::Left, {
                        cx.listener(move |pane, _event, _window, cx| {
                            pane.activate_terminal(i);
                            cx.notify();
                        })
                    });

                if is_active {
                    tab = tab.bg(rgb(theme::BG_PRIMARY)).text_color(rgb(theme::TEXT_PRIMARY));
                } else {
                    tab = tab.text_color(rgb(theme::TEXT_DIM));
                }

                tab_bar = tab_bar.child(tab.child(format!("Terminal {}", i + 1)));
            }

            // + button
            tab_bar = tab_bar.child(
                div()
                    .px(px(8.0))
                    .h_full()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .text_size(px(14.0))
                    .text_color(rgb(theme::TEXT_DIM))
                    .on_mouse_down(MouseButton::Left, cx.listener(|pane, _event, _window, cx| {
                        pane.add_terminal(&mut **cx);
                        cx.notify();
                    }))
                    .child("+"),
            );

            container = container.child(tab_bar);
        }

        container = container.child(
            div().flex_1().overflow_hidden().child(self.active_terminal().clone()),
        );

        container
    }
}
