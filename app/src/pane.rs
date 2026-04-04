use gpui::*;
use gpui_terminal::TerminalView;

use crate::root_view::{SplitDown, SplitRight};
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

    pub fn close_terminal(&mut self, idx: usize) -> bool {
        if self.terminals.len() <= 1 {
            return true; // pane should be removed
        }
        self.terminals.remove(idx);
        if self.active_idx >= self.terminals.len() {
            self.active_idx = self.terminals.len() - 1;
        }
        false
    }

    pub fn close_active_terminal(&mut self) -> bool {
        self.close_terminal(self.active_idx)
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

        // Header bar with tabs + action buttons
        let mut header = div()
            .flex()
            .flex_row()
            .items_center()
            .h(px(30.0))
            .bg(rgb(theme::BG_SECONDARY))
            .border_b_1()
            .border_color(rgb(theme::BORDER));

        // Terminal tabs
        let mut tabs_area = div().flex().flex_row().flex_1().overflow_hidden();
        for i in 0..self.terminals.len() {
            let is_active = i == self.active_idx;
            let mut tab = div()
                .id(ElementId::Name(format!("term-tab-{i}").into()))
                .flex()
                .items_center()
                .gap(px(6.0))
                .px(px(10.0))
                .h_full()
                .cursor_pointer()
                .text_size(px(12.0))
                .border_r_1()
                .border_color(rgb(theme::BORDER))
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

            tab = tab.child(format!("Terminal {}", i + 1));

            // Close button per tab (only if multiple)
            if self.terminals.len() > 1 {
                tab = tab.child(
                    div()
                        .id(ElementId::Name(format!("term-close-{i}").into()))
                        .text_size(px(10.0))
                        .text_color(rgb(theme::TEXT_DIM))
                        .cursor_pointer()
                        .hover(|s| s.text_color(rgb(theme::ACCENT_RED)))
                        .on_mouse_down(MouseButton::Left, {
                            cx.listener(move |pane, _event, _window, cx| {
                                pane.close_terminal(i);
                                cx.notify();
                            })
                        })
                        .child("x"),
                );
            }

            tabs_area = tabs_area.child(tab);
        }
        header = header.child(tabs_area);

        // Action buttons (right side of header)
        let actions = div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .px(px(4.0))
            .flex_shrink_0()
            // New terminal
            .child(
                div()
                    .id("pane-new-term")
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(px(3.0))
                    .text_size(px(12.0))
                    .text_color(rgb(theme::TEXT_DIM))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)).text_color(rgb(theme::TEXT_PRIMARY)))
                    .on_mouse_down(MouseButton::Left, cx.listener(|pane, _event, _window, cx| {
                        pane.add_terminal(&mut **cx);
                        cx.notify();
                    }))
                    .child("+"),
            )
            // Split right
            .child(
                div()
                    .id("pane-split-h")
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(px(3.0))
                    .text_size(px(11.0))
                    .text_color(rgb(theme::TEXT_DIM))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)).text_color(rgb(theme::TEXT_PRIMARY)))
                    .on_mouse_down(MouseButton::Left, cx.listener(|_pane, _event, window, cx| {
                        window.dispatch_action(Box::new(SplitRight), cx);
                    }))
                    .child("||"),
            )
            // Split down
            .child(
                div()
                    .id("pane-split-v")
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(px(3.0))
                    .text_size(px(11.0))
                    .text_color(rgb(theme::TEXT_DIM))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)).text_color(rgb(theme::TEXT_PRIMARY)))
                    .on_mouse_down(MouseButton::Left, cx.listener(|_pane, _event, window, cx| {
                        window.dispatch_action(Box::new(SplitDown), cx);
                    }))
                    .child("="),
            );

        header = header.child(actions);
        container = container.child(header);

        // Terminal content
        container = container.child(
            div().flex_1().overflow_hidden().child(self.active_terminal().clone()),
        );

        container
    }
}
