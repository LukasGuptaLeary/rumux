use gpui::*;

use crate::theme;

pub struct MenuItem {
    pub label: String,
    pub icon: Option<&'static str>,
    pub shortcut: Option<String>,
    pub separator_after: bool,
}

impl MenuItem {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            icon: None,
            shortcut: None,
            separator_after: false,
        }
    }

    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn shortcut(mut self, shortcut: &str) -> Self {
        self.shortcut = Some(shortcut.to_string());
        self
    }

    pub fn separator(mut self) -> Self {
        self.separator_after = true;
        self
    }
}

pub struct DropdownMenu {
    items: Vec<MenuItem>,
    selected: usize,
    on_select: Box<dyn Fn(usize, &mut Window, &mut App)>,
    pub focus_handle: FocusHandle,
}

impl DropdownMenu {
    pub fn new(
        items: Vec<MenuItem>,
        on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            items,
            selected: 0,
            on_select: Box::new(on_select),
            focus_handle: cx.focus_handle(),
        }
    }

    fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event.keystroke.key.as_str() {
            "escape" => {
                // Dismiss — parent should handle by setting menu to None
                cx.emit(MenuDismissed);
            }
            "up" => {
                if self.selected > 0 {
                    self.selected -= 1;
                    cx.notify();
                }
            }
            "down" => {
                if self.selected + 1 < self.items.len() {
                    self.selected += 1;
                    cx.notify();
                }
            }
            "enter" => {
                let idx = self.selected;
                (self.on_select)(idx, window, cx);
                cx.emit(MenuDismissed);
            }
            _ => {}
        }
    }
}

pub struct MenuDismissed;

impl EventEmitter<MenuDismissed> for DropdownMenu {}

impl Render for DropdownMenu {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div()
            .py(px(4.0))
            .flex()
            .flex_col();

        for (i, item) in self.items.iter().enumerate() {
            let is_selected = i == self.selected;
            let idx = i;

            let mut row = div()
                .id(ElementId::Name(format!("menu-item-{i}").into()))
                .px(px(12.0))
                .py(px(6.0))
                .flex()
                .items_center()
                .gap(px(8.0))
                .cursor_pointer()
                .text_size(px(13.0))
                .on_mouse_down(MouseButton::Left, cx.listener(move |menu, _event, window, cx| {
                    (menu.on_select)(idx, window, cx);
                    cx.emit(MenuDismissed);
                }));

            if is_selected {
                row = row.bg(rgb(theme::BG_HOVER)).text_color(rgb(theme::TEXT_PRIMARY));
            } else {
                row = row
                    .text_color(rgb(theme::TEXT_SECONDARY))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)).text_color(rgb(theme::TEXT_PRIMARY)));
            }

            // Icon
            if let Some(icon) = item.icon {
                row = row.child(
                    div()
                        .w(px(16.0))
                        .text_align(TextAlign::Center)
                        .child(icon.to_string()),
                );
            }

            // Label
            row = row.child(div().flex_1().child(item.label.clone()));

            // Shortcut
            if let Some(ref shortcut) = item.shortcut {
                row = row.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(theme::TEXT_DIM))
                        .px(px(4.0))
                        .py(px(1.0))
                        .bg(rgb(theme::BG_SURFACE))
                        .rounded(px(3.0))
                        .child(shortcut.clone()),
                );
            }

            list = list.child(row);

            // Separator
            if item.separator_after {
                list = list.child(
                    div()
                        .h(px(1.0))
                        .mx(px(8.0))
                        .my(px(4.0))
                        .bg(rgb(theme::BORDER)),
                );
            }
        }

        // Menu panel (positioned near top-right of parent)
        div()
            .absolute()
            .top(px(32.0))
            .right(px(4.0))
            .min_w(px(180.0))
            .bg(rgb(theme::BG_SECONDARY))
            .border_1()
            .border_color(rgb(theme::DIVIDER))
            .rounded(px(6.0))
            .overflow_hidden()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .child(list)
    }
}
