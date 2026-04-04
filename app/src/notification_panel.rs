use gpui::*;

use crate::app_state::AppState;
use crate::notifications::Notification;
use crate::theme;

pub struct NotificationPanel {
    app_state: Entity<AppState>,
    pub focus_handle: FocusHandle,
}

impl NotificationPanel {
    pub fn new(app_state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        Self {
            app_state,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for NotificationPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app_state.read(cx);
        let notifications: Vec<Notification> =
            state.notifications.iter().rev().cloned().collect();
        let count = notifications.len();

        let mut list = div().flex_1().overflow_hidden();

        if notifications.is_empty() {
            list = list.child(
                div()
                    .p(px(24.0))
                    .text_align(TextAlign::Center)
                    .text_color(rgb(theme::TEXT_DIM))
                    .child("No notifications"),
            );
        } else {
            for notif in &notifications {
                let mut entry = div()
                    .px(px(16.0))
                    .py(px(10.0))
                    .border_b_1()
                    .border_color(rgb(theme::BORDER));

                if !notif.read {
                    entry = entry.bg(Hsla {
                        h: 0.6,
                        s: 0.5,
                        l: 0.2,
                        a: 0.1,
                    });
                }

                let time_str = format_timestamp(notif.timestamp);

                entry = entry
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .mb(px(2.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(if notif.read {
                                        FontWeight::NORMAL
                                    } else {
                                        FontWeight::SEMIBOLD
                                    })
                                    .child(notif.title.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(rgb(theme::TEXT_DIM))
                                    .child(time_str),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(theme::TEXT_DIM))
                            .mt(px(2.0))
                            .child(notif.body.clone()),
                    );

                list = list.child(entry);
            }
        }

        div()
            .absolute()
            .right_0()
            .top_0()
            .bottom_0()
            .w(px(350.0))
            .bg(rgb(theme::BG_SECONDARY))
            .border_l_1()
            .border_color(rgb(theme::BORDER))
            .flex()
            .flex_col()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|_panel, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    window.dispatch_action(
                        Box::new(crate::root_view::ToggleNotificationPanel),
                        cx,
                    );
                }
            }))
            // Header
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(rgb(theme::BORDER))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("Notifications ({count})")),
                    )
                    .child(
                        div()
                            .id("clear-notifs")
                            .text_size(px(12.0))
                            .text_color(rgb(theme::TEXT_DIM))
                            .cursor_pointer()
                            .hover(|s| s.text_color(rgb(theme::TEXT_PRIMARY)))
                            .on_mouse_down(MouseButton::Left, {
                                let app_state = self.app_state.clone();
                                cx.listener(move |_panel, _event, _window, cx| {
                                    app_state.update(cx, |state, cx| {
                                        state.notifications.clear();
                                        cx.notify();
                                    });
                                })
                            })
                            .child("Clear all"),
                    ),
            )
            // List
            .child(list)
    }
}

fn format_timestamp(ts: u64) -> String {
    let secs = ts / 1000;
    let hours = (secs / 3600) % 24;
    let minutes = (secs / 60) % 60;
    let seconds = secs % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}
