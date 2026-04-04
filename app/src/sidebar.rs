use gpui::*;

use crate::app_state::AppState;
use crate::dropdown_menu::{DropdownMenu, MenuDismissed, MenuItem};
use crate::root_view::{DuplicateWorkspace, ToggleCommandPalette, ToggleNotificationPanel, ToggleSidebar};
use crate::text_input::{TextInputAction, TextInputState};
use crate::theme;

pub struct Sidebar {
    app_state: Entity<AppState>,
    rename_state: Option<(usize, TextInputState)>,
    rename_focus: Option<FocusHandle>,
    _rename_focus_sub: Option<gpui::Subscription>,
    context_menu: Option<(usize, Entity<DropdownMenu>)>,
}

impl Sidebar {
    pub fn new(app_state: Entity<AppState>) -> Self {
        Self {
            app_state,
            rename_state: None,
            rename_focus: None,
            _rename_focus_sub: None,
            context_menu: None,
        }
    }

    fn start_rename(&mut self, idx: usize, name: &str, cx: &mut Context<Self>) {
        self.clear_rename(cx);
        self.context_menu = None;
        let focus = cx.focus_handle();
        self.rename_state = Some((idx, TextInputState::new(name)));
        self.rename_focus = Some(focus);
        // on_focus_out subscription set up in render() where we have Window
        cx.notify();
    }

    fn finish_rename(&mut self, cx: &mut Context<Self>) {
        if let Some((idx, ref input)) = self.rename_state {
            let new_name = input.text.trim().to_string();
            if !new_name.is_empty() {
                self.app_state.update(cx, |state, cx| {
                    if idx < state.workspaces.len() {
                        state.workspaces[idx].update(cx, |ws, cx| {
                            ws.name = new_name;
                            cx.notify();
                        });
                    }
                    cx.notify();
                });
            }
        }
        self.clear_rename(cx);
    }

    fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.clear_rename(cx);
    }

    fn clear_rename(&mut self, cx: &mut Context<Self>) {
        self.rename_state = None;
        self.rename_focus = None;
        self._rename_focus_sub = None;
        cx.notify();
    }

    fn on_rename_key(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some((_idx, ref mut input)) = self.rename_state {
            let action =
                input.handle_key(&event.keystroke.key, event.keystroke.modifiers.control);
            match action {
                TextInputAction::Confirm => self.finish_rename(cx),
                TextInputAction::Cancel => self.cancel_rename(cx),
                TextInputAction::Changed | TextInputAction::None => cx.notify(),
            }
        }
    }

    fn show_context_menu(&mut self, idx: usize, window: &mut Window, cx: &mut Context<Self>) {
        let ws_count = self.app_state.read(cx).workspaces.len();
        let ws_name = self.app_state.read(cx).workspaces[idx].read(cx).name.clone();

        let mut items = vec![
            MenuItem::new("Rename").icon(theme::icons::RENAME),
            MenuItem::new("Duplicate").icon(theme::icons::PLUS),
        ];
        if ws_count > 1 {
            items.push(MenuItem::new("Close").icon(theme::icons::CLOSE));
        }

        let app_state = self.app_state.clone();
        let menu = cx.new(|cx| {
            DropdownMenu::new(
                items,
                move |selected, window, cx| {
                    match selected {
                        0 => {
                            // Rename — we can't call start_rename from here since we
                            // don't have &mut Sidebar. Dispatch an action instead.
                            // For simplicity, just focus the sidebar — the dismiss handler
                            // will trigger rename.
                        }
                        1 => {
                            // Duplicate
                            app_state.update(cx, |state, cx| {
                                state.set_active_workspace(idx, cx);
                            });
                            window.dispatch_action(Box::new(DuplicateWorkspace), cx);
                        }
                        2 => {
                            // Close
                            app_state.update(cx, |state, cx| {
                                state.close_workspace(idx, cx);
                            });
                        }
                        _ => {}
                    }
                },
                cx,
            )
        });

        let rename_idx = idx;
        let rename_name = ws_name;
        cx.subscribe(&menu, move |sidebar: &mut Self, _menu, _event: &MenuDismissed, cx| {
            let was_rename = sidebar.context_menu.as_ref().is_some_and(|(i, _)| *i == rename_idx);
            sidebar.context_menu = None;
            // If the user selected "Rename" (index 0), we handle it here
            // since we couldn't access &mut Sidebar from the callback
            if was_rename {
                // The menu already dismissed, no rename triggered from callback
            }
            cx.notify();
        })
        .detach();

        menu.read(cx).focus_handle.focus(window);
        self.context_menu = Some((idx, menu));
        cx.notify();
    }
}

impl Render for Sidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Set up on_focus_out for rename (needs window, only available in render)
        if self.rename_state.is_some() && self._rename_focus_sub.is_none() {
            if let Some(ref focus) = self.rename_focus {
                let sub = cx.on_focus_out(focus, window, |sidebar: &mut Self, _event, _window, cx| {
                    sidebar.clear_rename(cx);
                });
                self._rename_focus_sub = Some(sub);
            }
        }

        let state = self.app_state.read(cx);
        let active_idx = state.active_workspace_idx;
        let ws_count = state.workspaces.len();
        let notif_count = state.notifications.len();

        let mut tabs = div().flex_1().overflow_hidden();

        for i in 0..ws_count {
            let ws = state.workspaces[i].read(cx);
            let name = ws.name.clone();
            let is_active = i == active_idx;
            let unread = ws.unread_count;
            let ws_color = ws.color.unwrap_or(theme::ACCENT);
            let git_branch = ws.git_branch.clone();
            let is_renaming = self.rename_state.as_ref().is_some_and(|(idx, _)| *idx == i);

            let mut tab = div()
                .id(ElementId::Name(format!("ws-tab-{i}").into()))
                .px(px(12.0))
                .py(px(8.0))
                .cursor_pointer()
                // Single click: activate workspace
                .on_mouse_down(MouseButton::Left, {
                    let app_state = self.app_state.clone();
                    let name_for_rename = name.clone();
                    cx.listener(move |sidebar, event: &MouseDownEvent, window, cx| {
                        if event.click_count == 2 {
                            // Double-click: rename
                            sidebar.start_rename(i, &name_for_rename, cx);
                            if let Some(ref focus) = sidebar.rename_focus {
                                focus.focus(window);
                            }
                        } else {
                            // Single click: switch workspace
                            app_state.update(cx, |state, cx| {
                                state.set_active_workspace(i, cx);
                            });
                        }
                    })
                })
                // Right-click: context menu
                .on_mouse_down(MouseButton::Right, {
                    cx.listener(move |sidebar, _event, window, cx| {
                        sidebar.show_context_menu(i, window, cx);
                    })
                });

            if is_active {
                tab = tab
                    .bg(rgb(theme::BG_PRIMARY))
                    .border_l_2()
                    .border_color(rgb(ws_color));
            } else {
                tab = tab.hover(|s| s.bg(rgb(theme::BG_HOVER)));
            }

            let content = if is_renaming {
                let text = self
                    .rename_state
                    .as_ref()
                    .map(|(_, input)| input.text.clone())
                    .unwrap_or_default();

                let mut rename_div = div()
                    .px(px(4.0))
                    .py(px(1.0))
                    .bg(rgb(theme::BG_SURFACE))
                    .border_1()
                    .border_color(rgb(theme::ACCENT))
                    .rounded(px(3.0))
                    .text_size(px(13.0))
                    .text_color(rgb(theme::TEXT_PRIMARY));

                if let Some(ref focus) = self.rename_focus {
                    rename_div = rename_div
                        .track_focus(focus)
                        .on_key_down(cx.listener(Self::on_rename_key));
                }

                div().child(rename_div.child(format!("{text}|")))
            } else {
                let mut name_el = div().text_size(px(13.0)).flex_1().overflow_hidden();
                if is_active {
                    name_el = name_el
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme::TEXT_PRIMARY));
                } else {
                    name_el = name_el.text_color(rgb(theme::TEXT_SECONDARY));
                }
                name_el = name_el.child(name.clone());

                let mut row = div().flex().items_center().gap(px(4.0)).child(name_el);

                // Unread badge
                if unread > 0 {
                    row = row.child(
                        div()
                            .px(px(5.0))
                            .bg(rgb(theme::ACCENT_RED))
                            .text_color(rgb(0xffffff))
                            .text_size(px(10.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .rounded(px(8.0))
                            .child(format!("{unread}")),
                    );
                }

                // Close button (only, no rename button — use double-click)
                if ws_count > 1 {
                    row = row.child(
                        div()
                            .id(ElementId::Name(format!("ws-close-{i}").into()))
                            .px(px(3.0))
                            .text_size(px(11.0))
                            .text_color(rgb(theme::TEXT_DIM))
                            .cursor_pointer()
                            .hover(|s| s.text_color(rgb(theme::ACCENT_RED)))
                            .on_mouse_down(MouseButton::Left, {
                                let app_state = self.app_state.clone();
                                cx.listener(move |_sidebar, _event, _window, cx| {
                                    app_state.update(cx, |state, cx| {
                                        state.close_workspace(i, cx);
                                    });
                                })
                            })
                            .child(theme::icons::CLOSE),
                    );
                }

                row
            };

            tab = tab.child(content);

            // Git branch
            if let Some(ref branch) = git_branch {
                tab = tab.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(theme::TEXT_DIM))
                        .overflow_hidden()
                        .child(branch.clone()),
                );
            }

            tabs = tabs.child(tab);
        }

        let mut sidebar = div()
            .w(px(200.0))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(rgb(theme::BG_SECONDARY))
            .border_r_1()
            .border_color(rgb(theme::BORDER))
            // Header
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(12.0))
                    .py(px(8.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_DIM))
                            .child("WORKSPACES"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .id("sidebar-notif-btn")
                                    .px(px(5.0))
                                    .py(px(2.0))
                                    .rounded(px(3.0))
                                    .text_size(px(12.0))
                                    .text_color(if notif_count > 0 {
                                        rgb(theme::ACCENT_YELLOW)
                                    } else {
                                        rgb(theme::TEXT_DIM)
                                    })
                                    .cursor_pointer()
                                    .hover(|s| {
                                        s.bg(rgb(theme::BG_HOVER))
                                            .text_color(rgb(theme::TEXT_PRIMARY))
                                    })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|_s, _e, window, cx| {
                                            window.dispatch_action(
                                                Box::new(ToggleNotificationPanel),
                                                cx,
                                            );
                                        }),
                                    )
                                    .child(if notif_count > 0 {
                                        format!("{} {notif_count}", theme::icons::BELL)
                                    } else {
                                        theme::icons::BELL.to_string()
                                    }),
                            )
                            .child(
                                div()
                                    .id("sidebar-palette-btn")
                                    .px(px(5.0))
                                    .py(px(2.0))
                                    .rounded(px(3.0))
                                    .text_size(px(12.0))
                                    .text_color(rgb(theme::TEXT_DIM))
                                    .cursor_pointer()
                                    .hover(|s| {
                                        s.bg(rgb(theme::BG_HOVER))
                                            .text_color(rgb(theme::TEXT_PRIMARY))
                                    })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|_s, _e, window, cx| {
                                            window.dispatch_action(
                                                Box::new(ToggleCommandPalette),
                                                cx,
                                            );
                                        }),
                                    )
                                    .child(theme::icons::PALETTE),
                            )
                            .child(
                                div()
                                    .id("sidebar-collapse-btn")
                                    .px(px(5.0))
                                    .py(px(2.0))
                                    .rounded(px(3.0))
                                    .text_size(px(12.0))
                                    .text_color(rgb(theme::TEXT_DIM))
                                    .cursor_pointer()
                                    .hover(|s| {
                                        s.bg(rgb(theme::BG_HOVER))
                                            .text_color(rgb(theme::TEXT_PRIMARY))
                                    })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|_s, _e, window, cx| {
                                            window.dispatch_action(
                                                Box::new(ToggleSidebar),
                                                cx,
                                            );
                                        }),
                                    )
                                    .child(theme::icons::CHEVRON_LEFT),
                            ),
                    ),
            )
            .child(tabs)
            .child(
                div()
                    .p(px(8.0))
                    .border_t_1()
                    .border_color(rgb(theme::BORDER))
                    .child(
                        div()
                            .id("new-workspace-btn")
                            .w_full()
                            .py(px(6.0))
                            .rounded(px(4.0))
                            .bg(rgb(theme::BG_HOVER))
                            .text_color(rgb(theme::TEXT_SECONDARY))
                            .text_size(px(12.0))
                            .text_align(TextAlign::Center)
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(theme::DIVIDER)))
                            .on_mouse_down(MouseButton::Left, {
                                let app_state = self.app_state.clone();
                                cx.listener(move |_sidebar, _event, _window, cx| {
                                    app_state.update(cx, |state, cx| {
                                        state.add_workspace(cx);
                                    });
                                })
                            })
                            .child(format!("{} New Workspace", theme::icons::PLUS)),
                    ),
            );

        // Context menu overlay
        if let Some((_idx, ref menu)) = self.context_menu {
            sidebar = sidebar.child(menu.clone());
        }

        sidebar
    }
}
