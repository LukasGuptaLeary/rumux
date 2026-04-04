use gpui::*;

use crate::app_state::AppState;
use crate::root_view::{ToggleCommandPalette, ToggleNotificationPanel, ToggleSidebar};
use crate::theme;

pub struct Sidebar {
    app_state: Entity<AppState>,
}

impl Sidebar {
    pub fn new(app_state: Entity<AppState>) -> Self {
        Self { app_state }
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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

            let mut tab = div()
                .id(ElementId::Name(format!("ws-tab-{i}").into()))
                .px(px(12.0))
                .py(px(8.0))
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, {
                    let app_state = self.app_state.clone();
                    cx.listener(move |_sidebar, _event, _window, cx| {
                        app_state.update(cx, |state, cx| {
                            state.set_active_workspace(i, cx);
                        });
                    })
                });

            if is_active {
                tab = tab
                    .bg(rgb(theme::BG_PRIMARY))
                    .border_l_2()
                    .border_color(rgb(ws_color));
            }

            // Name
            let mut name_el = div().text_size(px(13.0)).flex_1().overflow_hidden();
            if is_active {
                name_el = name_el
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_PRIMARY));
            } else {
                name_el = name_el.text_color(rgb(theme::TEXT_SECONDARY));
            }
            name_el = name_el.child(name);

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

            // Close button
            if ws_count > 1 {
                row = row.child(
                    div()
                        .id(ElementId::Name(format!("ws-close-{i}").into()))
                        .px(px(4.0))
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
                        .child("x"),
                );
            }

            tab = tab.child(row);
            tabs = tabs.child(tab);
        }

        div()
            .w(px(200.0))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(rgb(theme::BG_SECONDARY))
            .border_r_1()
            .border_color(rgb(theme::BORDER))
            // Header with action buttons
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
                            // Notification bell
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
                                        cx.listener(|_sidebar, _event, window, cx| {
                                            window.dispatch_action(
                                                Box::new(ToggleNotificationPanel),
                                                cx,
                                            );
                                        }),
                                    )
                                    .child(if notif_count > 0 {
                                        format!("! {notif_count}")
                                    } else {
                                        "!".to_string()
                                    }),
                            )
                            // Command palette
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
                                        cx.listener(|_sidebar, _event, window, cx| {
                                            window.dispatch_action(
                                                Box::new(ToggleCommandPalette),
                                                cx,
                                            );
                                        }),
                                    )
                                    .child(">_"),
                            )
                            // Collapse sidebar
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
                                        cx.listener(|_sidebar, _event, window, cx| {
                                            window.dispatch_action(
                                                Box::new(ToggleSidebar),
                                                cx,
                                            );
                                        }),
                                    )
                                    .child("<"),
                            ),
                    ),
            )
            // Workspace tabs
            .child(tabs)
            // New workspace button
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
                            .child("+ New Workspace"),
                    ),
            )
    }
}
