use gpui::*;

use crate::root_view::ToggleFindBar;
use crate::text_input::{TextInputAction, TextInputState};
use crate::theme;

pub struct FindBar {
    input: TextInputState,
    pub focus_handle: FocusHandle,
    match_count: usize,
}

impl FindBar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            input: TextInputState::new(""),
            focus_handle: cx.focus_handle(),
            match_count: 0,
        }
    }

    pub fn query(&self) -> &str {
        &self.input.text
    }

    fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self
            .input
            .handle_key(&event.keystroke.key, event.keystroke.modifiers.control)
        {
            TextInputAction::Confirm | TextInputAction::Cancel => {
                window.dispatch_action(Box::new(ToggleFindBar), cx);
            }
            TextInputAction::Changed => cx.notify(),
            TextInputAction::None => {}
        }
    }
}

impl Render for FindBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.input.text.clone();
        let has_query = !query.is_empty();

        div()
            .absolute()
            .top_0()
            .right(px(80.0))
            .bg(rgb(theme::BG_SECONDARY))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .rounded_b(px(6.0))
            .px(px(10.0))
            .py(px(6.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .child(
                div()
                    .min_w(px(200.0))
                    .px(px(8.0))
                    .py(px(3.0))
                    .bg(rgb(theme::BG_SURFACE))
                    .border_1()
                    .border_color(if has_query {
                        rgb(theme::ACCENT)
                    } else {
                        rgb(theme::BORDER)
                    })
                    .rounded(px(4.0))
                    .text_size(px(13.0))
                    .child(if query.is_empty() {
                        div()
                            .text_color(rgb(theme::TEXT_DIM))
                            .child("Find...")
                    } else {
                        div()
                            .text_color(rgb(theme::TEXT_PRIMARY))
                            .child(format!("{query}|"))
                    }),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(theme::TEXT_DIM))
                    .child(if has_query {
                        format!("{} matches", self.match_count)
                    } else {
                        String::new()
                    }),
            )
            .child(
                div()
                    .id("find-close")
                    .text_size(px(12.0))
                    .text_color(rgb(theme::TEXT_DIM))
                    .cursor_pointer()
                    .hover(|s| s.text_color(rgb(theme::TEXT_PRIMARY)))
                    .on_mouse_down(MouseButton::Left, cx.listener(|_bar, _event, window, cx| {
                        window.dispatch_action(Box::new(ToggleFindBar), cx);
                    }))
                    .child("x"),
            )
    }
}
