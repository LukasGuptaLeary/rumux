use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::IconName;
use gpui_component::{Selectable, Sizable};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::tab::{Tab, TabBar};
use gpui_terminal::TerminalView;

use crate::root_view::{SplitDown, SplitRight, TogglePaneZoom};
use crate::terminal_surface::spawn_terminal_view;
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
    rename_idx: Option<usize>,
    rename_editor: Option<Entity<InputState>>,
    _rename_sub: Option<gpui::Subscription>,
    pub needs_focus: bool,
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
            rename_idx: None,
            rename_editor: None,
            _rename_sub: None,
            needs_focus: true,
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

    fn start_rename(&mut self, idx: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.clear_rename();

        let current = self.tab_name(idx);
        let editor = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(&current, window, cx);
            state.focus(window, cx);
            state
        });

        let sub = cx.subscribe(
            &editor,
            move |pane: &mut Self, editor, event: &InputEvent, cx| match event {
                InputEvent::PressEnter { .. } => {
                    let text = editor.read(cx).text().to_string().trim().to_string();
                    if let Some(rename_idx) = pane.rename_idx {
                        if rename_idx < pane.names.len() {
                            pane.names[rename_idx] =
                                if text.is_empty() { None } else { Some(text) };
                        }
                    }
                    pane.clear_rename();
                    pane.needs_focus = true;
                    cx.notify();
                }
                InputEvent::Blur => {
                    pane.clear_rename();
                    pane.needs_focus = true;
                    cx.notify();
                }
                _ => {}
            },
        );

        self.rename_idx = Some(idx);
        self.rename_editor = Some(editor);
        self._rename_sub = Some(sub);
        cx.notify();
    }

    fn clear_rename(&mut self) {
        self.rename_idx = None;
        self.rename_editor = None;
        self._rename_sub = None;
    }
}

impl Render for Pane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_rename = self.rename_editor.is_some();

        if !has_rename {
            let should_focus =
                self.needs_focus || self.focus_handle.contains_focused(window, cx);
            if should_focus {
                self.needs_focus = false;
                self.active_terminal()
                    .read(cx)
                    .focus_handle()
                    .focus(window);
            }
        }

        let mut container = div().size_full().flex().flex_col().track_focus(&self.focus_handle);

        // Terminal tabs using gpui-component TabBar (IS the header)
        let mut tab_bar = TabBar::new("pane-tabs")
            .xsmall()
            .selected_index(self.active_idx)
            .on_click(cx.listener(|pane, idx: &usize, _window, cx| {
                pane.activate_terminal(*idx);
                cx.notify();
            }));

        for i in 0..self.terminals.len() {
            let is_renaming = self.rename_idx == Some(i);
            let tab_name = self.tab_name(i);

            let is_active = i == self.active_idx;
            let mut tab = Tab::new()
                .icon(IconName::SquareTerminal)
                .selected(is_active);

            if is_renaming {
                if let Some(ref editor) = self.rename_editor {
                    tab = tab.label("").suffix(
                        Input::new(editor)
                            .appearance(false)
                            .bordered(false)
                            .xsmall(),
                    );
                }
            } else {
                tab = tab.label(SharedString::from(tab_name));

                if self.terminals.len() > 1 {
                    tab = tab.suffix(
                        Button::new(SharedString::from(format!("close-tab-{i}")))
                            .ghost()
                            .compact()
                            .icon(IconName::Close)
                            .on_click(cx.listener(move |pane, _event, _window, cx| {
                                pane.close_terminal(i);
                                cx.notify();
                            })),
                    );
                }
            }

            tab_bar = tab_bar.child(tab);
        }

        // Put action buttons in the TabBar suffix
        let action_buttons = div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .px(px(4.0))
            .child(
                Button::new("pane-new-term")
                    .ghost()
                    .compact()
                    .icon(IconName::Plus)
                    .on_click(cx.listener(|pane, _event, _window, cx| {
                        pane.add_terminal(&mut **cx, None);
                        cx.notify();
                    })),
            )
            .child(
                Button::new("pane-split-h")
                    .ghost()
                    .compact()
                    .icon(IconName::PanelRight)
                    .on_click(cx.listener(|_pane, _event, window, cx| {
                        window.dispatch_action(Box::new(SplitRight), cx);
                    })),
            )
            .child(
                Button::new("pane-split-v")
                    .ghost()
                    .compact()
                    .icon(IconName::PanelBottom)
                    .on_click(cx.listener(|_pane, _event, window, cx| {
                        window.dispatch_action(Box::new(SplitDown), cx);
                    })),
            );

        // Add zoom, agent buttons to the action buttons div
        let mut action_buttons = action_buttons;

        if self.can_zoom || self.is_zoomed {
            let icon = if self.is_zoomed {
                IconName::Minimize
            } else {
                IconName::Maximize
            };
            let mut btn = Button::new("pane-zoom").compact().icon(icon);
            if self.is_zoomed {
                btn = btn.primary();
            } else {
                btn = btn.ghost();
            }
            action_buttons = action_buttons.child(btn.on_click(cx.listener(
                |_pane, _event, window, cx| {
                    window.dispatch_action(Box::new(TogglePaneZoom), cx);
                },
            )));
        }

        action_buttons = action_buttons.child(
            Button::new("pane-agent")
                .ghost()
                .compact()
                .icon(IconName::Bot)
                .on_click(cx.listener(|pane, _event, _window, cx| {
                    if let Some(term) = pane.terminals.get(pane.active_idx) {
                        term.update(cx, |view, _cx| {
                            view.write_to_pty(b"claude\n");
                        });
                    }
                })),
        );

        if self.is_zoomed {
            action_buttons = action_buttons.child(
                div()
                    .px(px(6.0))
                    .py(px(2.0))
                    .bg(rgb(theme::ACCENT))
                    .text_color(rgb(theme::BG_PRIMARY))
                    .text_size(px(10.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .rounded(px(10.0))
                    .child("Zoomed"),
            );
        }

        tab_bar = tab_bar.suffix(action_buttons);

        container = container.child(tab_bar);
        container = container.child(
            div()
                .flex_1()
                .overflow_hidden()
                .child(self.active_terminal().clone()),
        );

        container
    }
}
