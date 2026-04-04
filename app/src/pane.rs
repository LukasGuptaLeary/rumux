use gpui::*;
use gpui_terminal::TerminalView;

use crate::dropdown_menu::{DropdownMenu, MenuDismissed, MenuItem};
use crate::root_view::{SplitDown, SplitRight, TogglePaneZoom};
use crate::terminal_surface::spawn_terminal_view;
use crate::text_input::{TextInputAction, TextInputState};
use crate::theme;

const AGENTS: &[(&str, &str)] = &[
    ("Claude Code", "claude\n"),
    ("Codex", "codex\n"),
    ("OpenCode", "opencode\n"),
];

pub struct Pane {
    terminals: Vec<Entity<TerminalView>>,
    names: Vec<Option<String>>,
    active_idx: usize,
    focus_handle: FocusHandle,
    pub is_zoomed: bool,
    pub can_zoom: bool,
    rename_state: Option<(usize, TextInputState)>,
    rename_focus: Option<FocusHandle>,
    agent_menu: Option<Entity<DropdownMenu>>,
}

impl Pane {
    pub fn new(terminal: Entity<TerminalView>, cx: &mut Context<Self>) -> Self {
        Self {
            terminals: vec![terminal],
            names: vec![None],
            active_idx: 0,
            focus_handle: cx.focus_handle(),
            is_zoomed: false,
            can_zoom: false,
            rename_state: None,
            rename_focus: None,
            agent_menu: None,
        }
    }

    pub fn active_terminal(&self) -> &Entity<TerminalView> {
        &self.terminals[self.active_idx]
    }

    pub fn terminal_count(&self) -> usize {
        self.terminals.len()
    }

    pub fn add_terminal(&mut self, cx: &mut App, cwd: Option<&std::path::Path>) {
        if let Ok(term) = spawn_terminal_view(cx, cwd, None) {
            self.terminals.push(term);
            self.names.push(None);
            self.active_idx = self.terminals.len() - 1;
        }
    }

    pub fn close_terminal(&mut self, idx: usize) -> bool {
        if self.terminals.len() <= 1 {
            return true;
        }
        self.terminals.remove(idx);
        self.names.remove(idx);
        if self.active_idx >= self.terminals.len() {
            self.active_idx = self.terminals.len() - 1;
        }
        false
    }

    pub fn close_active_terminal(&mut self) -> bool {
        self.close_terminal(self.active_idx)
    }

    pub fn close_others(&mut self, keep_idx: usize) {
        if keep_idx >= self.terminals.len() {
            return;
        }
        let term = self.terminals.remove(keep_idx);
        let name = self.names.remove(keep_idx);
        self.terminals.clear();
        self.names.clear();
        self.terminals.push(term);
        self.names.push(name);
        self.active_idx = 0;
    }

    pub fn close_to_right(&mut self, idx: usize) {
        if idx + 1 < self.terminals.len() {
            self.terminals.truncate(idx + 1);
            self.names.truncate(idx + 1);
            if self.active_idx > idx {
                self.active_idx = idx;
            }
        }
    }

    pub fn close_to_left(&mut self, idx: usize) {
        if idx > 0 && idx < self.terminals.len() {
            self.terminals.drain(..idx);
            self.names.drain(..idx);
            self.active_idx = self.active_idx.saturating_sub(idx);
        }
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

    fn tab_name(&self, idx: usize) -> String {
        self.names[idx]
            .clone()
            .unwrap_or_else(|| format!("Terminal {}", idx + 1))
    }

    fn start_rename(&mut self, idx: usize, cx: &mut Context<Self>) {
        let current = self.tab_name(idx);
        self.rename_state = Some((idx, TextInputState::new(&current)));
        self.rename_focus = Some(cx.focus_handle());
        cx.notify();
    }

    fn finish_rename(&mut self, cx: &mut Context<Self>) {
        if let Some((idx, ref input)) = self.rename_state {
            let new = input.text.trim().to_string();
            if idx < self.names.len() {
                self.names[idx] = if new.is_empty() { None } else { Some(new) };
            }
        }
        self.rename_state = None;
        self.rename_focus = None;
        cx.notify();
    }

    fn toggle_agent_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.agent_menu.is_some() {
            self.agent_menu = None;
            cx.notify();
            return;
        }

        let items: Vec<MenuItem> = AGENTS
            .iter()
            .map(|(name, _)| MenuItem::new(name).icon(theme::icons::AGENT))
            .collect();

        let terminals = self.terminals.clone();
        let active_idx = self.active_idx;
        let menu = cx.new(|cx| {
            DropdownMenu::new(
                items,
                move |idx, _window, cx| {
                    if let Some((_, cmd)) = AGENTS.get(idx) {
                        if let Some(term) = terminals.get(active_idx) {
                            term.update(cx, |view, _cx| {
                                view.write_to_pty(cmd.as_bytes());
                            });
                        }
                    }
                },
                cx,
            )
        });

        cx.subscribe(&menu, |pane: &mut Self, _menu, _event: &MenuDismissed, cx| {
            pane.agent_menu = None;
            cx.notify();
        })
        .detach();

        menu.read(cx).focus_handle.focus(window);
        self.agent_menu = Some(menu);
        cx.notify();
    }

    fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.rename_state = None;
        self.rename_focus = None;
        cx.notify();
    }

    fn on_rename_key(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some((_idx, ref mut input)) = self.rename_state {
            match input.handle_key(&event.keystroke.key, event.keystroke.modifiers.control) {
                TextInputAction::Confirm => self.finish_rename(cx),
                TextInputAction::Cancel => self.cancel_rename(cx),
                _ => cx.notify(),
            }
        }
    }
}

impl Render for Pane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.active_terminal()
            .read(cx)
            .focus_handle()
            .focus(window);

        let mut container = div().size_full().flex().flex_col().track_focus(&self.focus_handle);

        // Header bar
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
            let is_renaming = self.rename_state.as_ref().is_some_and(|(idx, _)| *idx == i);

            let mut tab = div()
                .id(ElementId::Name(format!("term-tab-{i}").into()))
                .flex()
                .items_center()
                .gap(px(4.0))
                .px(px(8.0))
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

            if is_renaming {
                let text = self
                    .rename_state
                    .as_ref()
                    .map(|(_, input)| input.text.clone())
                    .unwrap_or_default();

                let mut rename_el = div()
                    .px(px(2.0))
                    .bg(rgb(theme::BG_SURFACE))
                    .border_1()
                    .border_color(rgb(theme::ACCENT))
                    .rounded(px(2.0))
                    .text_size(px(12.0))
                    .text_color(rgb(theme::TEXT_PRIMARY));

                if let Some(ref focus) = self.rename_focus {
                    rename_el = rename_el
                        .track_focus(focus)
                        .on_key_down(cx.listener(Self::on_rename_key));
                }

                tab = tab.child(rename_el.child(format!("{text}|")));
            } else {
                tab = tab.child(self.tab_name(i));

                // Rename button
                tab = tab.child(
                    div()
                        .id(ElementId::Name(format!("term-rename-{i}").into()))
                        .text_size(px(9.0))
                        .text_color(rgb(theme::TEXT_DIM))
                        .cursor_pointer()
                        .hover(|s| s.text_color(rgb(theme::ACCENT)))
                        .on_mouse_down(MouseButton::Left, {
                            cx.listener(move |pane, _event, window, cx| {
                                pane.start_rename(i, cx);
                                if let Some(ref focus) = pane.rename_focus {
                                    focus.focus(window);
                                }
                            })
                        })
                        .child("R"),
                );

                // Close button
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

                    // Context actions on active tab with 3+ terminals
                    if is_active && self.terminals.len() >= 3 {
                        // Close Others
                        tab = tab.child(
                            div()
                                .id(ElementId::Name(format!("term-close-others-{i}").into()))
                                .text_size(px(9.0))
                                .text_color(rgb(theme::TEXT_DIM))
                                .cursor_pointer()
                                .hover(|s| s.text_color(rgb(theme::ACCENT_RED)))
                                .on_mouse_down(MouseButton::Left, {
                                    cx.listener(move |pane, _event, _window, cx| {
                                        pane.close_others(i);
                                        cx.notify();
                                    })
                                })
                                .child("xO"),
                        );
                    }
                }
            }

            tabs_area = tabs_area.child(tab);
        }
        header = header.child(tabs_area);

        // Action buttons
        let mut actions = div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .px(px(4.0))
            .flex_shrink_0()
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
                        pane.add_terminal(&mut **cx, None);
                        cx.notify();
                    }))
                    .child("+"),
            )
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

        // Zoom toggle
        if self.can_zoom || self.is_zoomed {
            let is_zoomed = self.is_zoomed;
            let mut zoom_btn = div()
                .id("pane-zoom")
                .px(px(6.0))
                .py(px(2.0))
                .rounded(px(3.0))
                .text_size(px(11.0))
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, cx.listener(|_pane, _event, window, cx| {
                    window.dispatch_action(Box::new(TogglePaneZoom), cx);
                }));
            if is_zoomed {
                zoom_btn = zoom_btn
                    .bg(rgb(theme::ACCENT))
                    .text_color(rgb(theme::BG_PRIMARY));
            } else {
                zoom_btn = zoom_btn
                    .text_color(rgb(theme::TEXT_DIM))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)).text_color(rgb(theme::TEXT_PRIMARY)));
            }
            actions = actions.child(zoom_btn.child(
                if is_zoomed { theme::icons::MINIMIZE } else { theme::icons::MAXIMIZE }
            ));
        }

        // Agent launcher button
        actions = actions.child(
            div()
                .id("pane-agent")
                .px(px(6.0))
                .py(px(2.0))
                .rounded(px(3.0))
                .text_size(px(13.0))
                .text_color(rgb(theme::ACCENT_GREEN))
                .cursor_pointer()
                .hover(|s| s.bg(rgb(theme::BG_HOVER)).text_color(rgb(theme::TEXT_PRIMARY)))
                .on_mouse_down(MouseButton::Left, cx.listener(|pane, _event, window, cx| {
                    pane.toggle_agent_menu(window, cx);
                }))
                .child(theme::icons::AGENT),
        );

        header = header.child(actions);

        if self.is_zoomed {
            header = header.child(
                div()
                    .flex_shrink_0()
                    .px(px(8.0))
                    .py(px(2.0))
                    .mx(px(4.0))
                    .bg(rgb(theme::ACCENT))
                    .text_color(rgb(theme::BG_PRIMARY))
                    .text_size(px(10.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .rounded(px(10.0))
                    .child(format!("{} Zoomed", theme::icons::MAXIMIZE)),
            );
        }

        container = container.child(header);
        container = container.child(
            div().flex_1().overflow_hidden().child(self.active_terminal().clone()),
        );

        // Agent menu overlay
        if let Some(menu) = &self.agent_menu {
            container = container.child(menu.clone());
        }

        container
    }
}
