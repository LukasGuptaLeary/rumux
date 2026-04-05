use gpui::*;

use gpui_component::IconName;
use gpui_component::Sizable;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};

use crate::root_view::ToggleFindBar;
use crate::theme;

pub struct FindBar {
    input: Entity<InputState>,
    pub focus_handle: FocusHandle,
    match_count: usize,
    _sub: gpui::Subscription,
}

impl FindBar {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("Find...", window, cx);
            state
        });

        input.update(cx, |state, cx| {
            state.focus(window, cx);
        });

        let sub = cx.subscribe(
            &input,
            |_bar: &mut Self, _editor, event: &InputEvent, cx| match event {
                InputEvent::Change => {
                    cx.notify();
                }
                _ => {}
            },
        );

        Self {
            input,
            focus_handle,
            match_count: 0,
            _sub: sub,
        }
    }

    #[allow(dead_code)]
    pub fn query(&self, cx: &App) -> String {
        self.input.read(cx).text().to_string()
    }
}

impl Render for FindBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.input.read(cx).text().to_string();
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
            .child(
                div()
                    .min_w(px(200.0))
                    .child(Input::new(&self.input).appearance(false).xsmall()),
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
                Button::new("find-close")
                    .ghost()
                    .compact()
                    .icon(IconName::Close)
                    .on_click(cx.listener(|_bar, _event, window, cx| {
                        window.dispatch_action(Box::new(ToggleFindBar), cx);
                    })),
            )
    }
}
