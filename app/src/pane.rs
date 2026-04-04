use gpui::*;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_terminal::TerminalView;

use crate::dropdown_menu::{DropdownMenu, MenuDismissed, MenuItem};
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
    // Rename: uses gpui-component Input for proper focus/blur handling
    rename_idx: Option<usize>,
    rename_editor: Option<Entity<InputState>>,
    _rename_sub: Option<gpui::Subscription>,
    // Menus
    agent_menu: Option<Entity<DropdownMenu>>,
    tab_context_menu: Option<(usize, Entity<DropdownMenu>)>,
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
            agent_menu: None,
            tab_context_menu: None,
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
        // Cancel any existing rename/menus
        self.clear_rename();
        self.agent_menu = None;
        self.tab_context_menu = None;

        let current = self.tab_name(idx);
        let editor = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(current, window, cx);
            state
        });

        // Subscribe to editor events for blur (cancel) and enter (confirm)
        let sub = cx.subscribe(&editor, move |pane: &mut Self, editor, event: &InputEvent, cx| {
            match event {
                InputEvent::PressEnter { .. } => {
                    let text = editor.read(cx).text().to_string();
                    let text = text.trim().to_string();
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
                    // Cancel rename on blur (click away)
                    pane.clear_rename();
                    pane.needs_focus = true;
                    cx.notify();
                }
                _ => {}
            }
        });

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

    fn show_tab_context_menu(&mut self, idx: usize, window: &mut Window, cx: &mut Context<Self>) {
        let has_multiple = self.terminals.len() > 1;

        let mut items = vec![MenuItem::new("Rename").icon(theme::icons::RENAME)];
        if has_multiple {
            items.push(MenuItem::new("Close Others").icon(theme::icons::CLOSE_OTHERS));
            items.push(MenuItem::new("Close to Right"));
            items.push(MenuItem::new("Close to Left").separator());
            items.push(MenuItem::new("Close").icon(theme::icons::CLOSE));
        }

        let selected_action = std::sync::Arc::new(std::sync::Mutex::new(None::<usize>));
        let selected_clone = selected_action.clone();

        let menu = cx.new(|cx| {
            DropdownMenu::new(
                items,
                move |selected, _window, _cx| {
                    if let Ok(mut s) = selected_clone.lock() {
                        *s = Some(selected);
                    }
                },
                cx,
            )
        });

        cx.subscribe(&menu, move |pane: &mut Self, _menu, _event: &MenuDismissed, cx| {
            let action = selected_action.lock().ok().and_then(|s| *s);
            pane.tab_context_menu = None;
            pane.needs_focus = true;

            if let Some(selected) = action {
                if has_multiple {
                    match selected {
                        0 => {} // Rename — needs window, handled below
                        1 => {
                            pane.close_others(idx);
                            cx.notify();
                        }
                        2 => {
                            pane.close_to_right(idx);
                            cx.notify();
                        }
                        3 => {
                            pane.close_to_left(idx);
                            cx.notify();
                        }
                        4 => {
                            pane.close_terminal(idx);
                            cx.notify();
                        }
                        _ => {}
                    }
                }
                // Rename from context menu can't be done here (no window).
                // User can double-click to rename instead.
            }

            cx.notify();
        })
        .detach();

        menu.read(cx).focus_handle.focus(window);
        self.tab_context_menu = Some((idx, menu));
        cx.notify();
    }

    fn toggle_agent_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.agent_menu.is_some() {
            self.agent_menu = None;
            self.needs_focus = true;
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
            pane.needs_focus = true;
            cx.notify();
        })
        .detach();

        menu.read(cx).focus_handle.focus(window);
        self.agent_menu = Some(menu);
        cx.notify();
    }
}

impl Render for Pane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Focus architecture:
        //   Rename editor manages its own focus (gpui-component Input handles it).
        //   Terminal gets focus only when no rename/menu is active.
        let has_rename = self.rename_editor.is_some();
        let has_menu = self.agent_menu.is_some() || self.tab_context_menu.is_some();

        if !has_rename && !has_menu {
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
            let is_renaming = self.rename_idx == Some(i);
            let tab_name = self.tab_name(i);

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
                    cx.listener(move |pane, event: &MouseDownEvent, window, cx| {
                        if event.click_count == 2 {
                            pane.start_rename(i, window, cx);
                        } else {
                            pane.activate_terminal(i);
                        }
                        cx.notify();
                    })
                })
                .on_mouse_down(MouseButton::Right, {
                    cx.listener(move |pane, _event, window, cx| {
                        pane.show_tab_context_menu(i, window, cx);
                    })
                });

            if is_active {
                tab = tab
                    .bg(rgb(theme::BG_PRIMARY))
                    .text_color(rgb(theme::TEXT_PRIMARY));
            } else {
                tab = tab
                    .text_color(rgb(theme::TEXT_DIM))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)));
            }

            if is_renaming {
                if let Some(ref editor) = self.rename_editor {
                    tab = tab.child(
                        Input::new(editor)
                            .appearance(false)
                            .bordered(false),
                    );
                }
            } else {
                tab = tab.child(tab_name);

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
                            .child(theme::icons::CLOSE),
                    );
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
                theme::icon_button("pane-new-term", theme::icons::PLUS).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|pane, _event, _window, cx| {
                        pane.add_terminal(&mut **cx, None);
                        cx.notify();
                    }),
                ),
            )
            .child(
                theme::icon_button("pane-split-h", theme::icons::SPLIT_H).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_pane, _event, window, cx| {
                        window.dispatch_action(Box::new(SplitRight), cx);
                    }),
                ),
            )
            .child(
                theme::icon_button("pane-split-v", theme::icons::SPLIT_V).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_pane, _event, window, cx| {
                        window.dispatch_action(Box::new(SplitDown), cx);
                    }),
                ),
            );

        // Zoom toggle
        if self.can_zoom || self.is_zoomed {
            let is_zoomed = self.is_zoomed;
            let zoom_btn = if is_zoomed {
                theme::icon_button_active("pane-zoom", theme::icons::MINIMIZE)
            } else {
                theme::icon_button("pane-zoom", theme::icons::MAXIMIZE)
            };
            actions = actions.child(zoom_btn.on_mouse_down(
                MouseButton::Left,
                cx.listener(|_pane, _event, window, cx| {
                    window.dispatch_action(Box::new(TogglePaneZoom), cx);
                }),
            ));
        }

        // Agent launcher
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
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|pane, _event, window, cx| {
                        pane.toggle_agent_menu(window, cx);
                    }),
                )
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
            div()
                .flex_1()
                .overflow_hidden()
                .child(self.active_terminal().clone()),
        );

        // Menu overlays
        if let Some(menu) = &self.agent_menu {
            container = container.child(menu.clone());
        }
        if let Some((_idx, menu)) = &self.tab_context_menu {
            container = container.child(menu.clone());
        }

        container
    }
}
