use gpui::*;

use crate::app_state::AppState;
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

        let mut tabs = div().flex_1().overflow_hidden();

        for i in 0..ws_count {
            let ws = state.workspaces[i].read(cx);
            let name = ws.name.clone();
            let is_active = i == active_idx;
            let unread = ws.unread_count;

            let mut tab = div()
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
                    .border_color(rgb(theme::ACCENT));
            }

            let mut name_el = div().text_size(px(13.0));
            if is_active {
                name_el = name_el
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_PRIMARY));
            } else {
                name_el = name_el.text_color(rgb(theme::TEXT_SECONDARY));
            }
            name_el = name_el.child(name);

            let mut row = div().flex().justify_between().child(name_el);

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
            .child(
                div()
                    .px(px(12.0))
                    .py(px(10.0))
                    .text_size(px(11.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_DIM))
                    .child("WORKSPACES"),
            )
            .child(tabs)
            .child(
                div()
                    .p(px(8.0))
                    .border_t_1()
                    .border_color(rgb(theme::BORDER))
                    .child(
                        div()
                            .w_full()
                            .py(px(6.0))
                            .rounded(px(4.0))
                            .bg(rgb(theme::BG_HOVER))
                            .text_color(rgb(theme::TEXT_SECONDARY))
                            .text_size(px(12.0))
                            .text_align(TextAlign::Center)
                            .cursor_pointer()
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
