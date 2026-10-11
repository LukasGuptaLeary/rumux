use gpui::*;
use gpui_terminal::TerminalView;

use gpui_component::IconName;
use gpui_component::Sizable;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};

use crate::root_view::ToggleFindBar;
use crate::theme;

pub struct FindBar {
    input: Entity<InputState>,
    pub focus_handle: FocusHandle,
    terminal: WeakEntity<TerminalView>,
    _sub: gpui::Subscription,
    _terminal_sub: gpui::Subscription,
}

impl FindBar {
    pub fn new(
        terminal: Entity<TerminalView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
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
            |bar: &mut Self, _editor, event: &InputEvent, cx| match event {
                InputEvent::Change => {
                    bar.search(0, cx);
                }
                InputEvent::PressEnter { secondary } => {
                    bar.search(if *secondary { -1 } else { 1 }, cx)
                }
                _ => {}
            },
        );

        let terminal_sub = cx.observe(&terminal, |_bar, _terminal, cx| cx.notify());
        Self {
            input,
            focus_handle,
            terminal: terminal.downgrade(),
            _sub: sub,
            _terminal_sub: terminal_sub,
        }
    }

    #[allow(dead_code)]
    pub fn query(&self, cx: &App) -> String {
        self.input.read(cx).text().to_string()
    }

    fn search(&mut self, advance: i32, cx: &mut Context<Self>) {
        let query = self.query(cx);
        if let Some(terminal) = self.terminal.upgrade() {
            terminal.update(cx, |terminal, cx| {
                terminal.find(&query, advance, cx);
            });
        }
        cx.notify();
    }

    pub fn clear(&self, cx: &mut App) {
        if let Some(terminal) = self.terminal.upgrade() {
            terminal.update(cx, |terminal, cx| {
                terminal.find("", 0, cx);
            });
        }
    }
}

impl Render for FindBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.input.read(cx).text().to_string();
        let has_query = !query.is_empty();
        let (match_count, active) = self
            .terminal
            .upgrade()
            .map(|terminal| terminal.read(cx).search_status())
            .unwrap_or_default();

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
            .on_key_down(cx.listener(|_bar, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    window.dispatch_action(Box::new(ToggleFindBar), cx);
                    cx.stop_propagation();
                }
            }))
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
                        format!(
                            "{} / {}{}",
                            active.map(|i| i + 1).unwrap_or(0),
                            match_count,
                            if match_count == 10_000 { "+" } else { "" }
                        )
                    } else {
                        String::new()
                    }),
            )
            .child(
                Button::new("find-prev")
                    .ghost()
                    .compact()
                    .icon(IconName::ChevronUp)
                    .tooltip("Previous match (Shift+Enter)")
                    .on_click(cx.listener(|bar, _, _, cx| bar.search(-1, cx))),
            )
            .child(
                Button::new("find-next")
                    .ghost()
                    .compact()
                    .icon(IconName::ChevronDown)
                    .tooltip("Next match (Enter)")
                    .on_click(cx.listener(|bar, _, _, cx| bar.search(1, cx))),
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
