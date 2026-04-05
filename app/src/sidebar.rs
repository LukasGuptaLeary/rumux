use gpui::*;
use gpui_component::IconName;
use gpui_component::Sizable;
use gpui_component::badge::Badge;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::menu::{ContextMenuExt, PopupMenuItem};

use crate::app_state::AppState;
use crate::root_view::{
    DuplicateWorkspace, ToggleCommandPalette, ToggleNotificationPanel, ToggleSidebar,
};
use crate::theme;

pub struct WorkspaceSidebar {
    app_state: Entity<AppState>,
    rename_idx: Option<usize>,
    rename_editor: Option<Entity<InputState>>,
    _rename_sub: Option<gpui::Subscription>,
}

impl WorkspaceSidebar {
    pub fn new(app_state: Entity<AppState>) -> Self {
        Self {
            app_state,
            rename_idx: None,
            rename_editor: None,
            _rename_sub: None,
        }
    }

    fn start_rename(
        &mut self,
        idx: usize,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clear_rename();

        let name_owned = name.to_string();
        let editor = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(&name_owned, window, cx);
            state
        });

        let app_state = self.app_state.clone();
        let sub = cx.subscribe(
            &editor,
            move |sidebar: &mut Self, editor, event: &InputEvent, cx| match event {
                InputEvent::PressEnter { .. } | InputEvent::Blur => {
                    let text = editor.read(cx).text().to_string();
                    let text = text.trim().to_string();
                    if let Some(rename_idx) = sidebar.rename_idx {
                        app_state.update(cx, |state, cx| {
                            state.rename_workspace(rename_idx, text.clone(), cx);
                        });
                    }
                    sidebar.clear_rename();
                    cx.notify();
                }
                _ => {}
            },
        );

        self.rename_idx = Some(idx);
        self.rename_editor = Some(editor);
        self._rename_sub = Some(sub);
        cx.notify();

        let Some(editor) = self.rename_editor.clone() else {
            return;
        };
        cx.spawn_in(window, async move |_, cx| {
            let _ = cx.update(|window, cx| {
                editor.update(cx, |state, cx| {
                    state.focus_and_select_all(window, cx);
                });
            });
        })
        .detach();
    }

    pub fn clear_rename(&mut self) {
        self.rename_idx = None;
        self.rename_editor = None;
        self._rename_sub = None;
    }
}

impl Render for WorkspaceSidebar {
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
            let ws_color = ws.color.unwrap_or(theme::ACCENT);
            let git_branch = ws.git_branch.clone();
            let is_renaming = self.rename_idx == Some(i);

            let mut tab = div()
                .id(ElementId::Name(format!("ws-tab-{i}").into()))
                .px(px(12.0))
                .py(px(8.0))
                .cursor_pointer();

            if !is_renaming {
                tab = tab.on_mouse_down(MouseButton::Left, {
                    let app_state = self.app_state.clone();
                    let name_for_rename = name.clone();
                    cx.listener(move |sidebar, event: &MouseDownEvent, window, cx| {
                        if event.click_count == 2 {
                            sidebar.start_rename(i, &name_for_rename, window, cx);
                        } else {
                            sidebar.clear_rename();
                            app_state.update(cx, |state, cx| {
                                state.set_active_workspace(i, window, cx);
                            });
                            cx.notify();
                        }
                    })
                });
            }

            if is_active {
                tab = tab
                    .bg(rgb(theme::BG_PRIMARY))
                    .border_l_2()
                    .border_color(rgb(ws_color));
            } else {
                tab = tab.hover(|s| s.bg(rgb(theme::BG_HOVER)));
            }

            let content = if is_renaming {
                if let Some(ref editor) = self.rename_editor {
                    div()
                        .w_full()
                        .h(px(22.0))
                        .flex()
                        .items_center()
                        .on_mouse_down(MouseButton::Left, |_event, _window, cx| {
                            cx.stop_propagation();
                        })
                        .child(
                            Input::new(editor)
                                .appearance(false)
                                .bordered(false)
                                .xsmall(),
                        )
                } else {
                    div()
                }
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

                if unread > 0 {
                    row = row.child(Badge::new().count(unread));
                }

                if ws_count > 1 {
                    row = row.child(
                        Button::new(SharedString::from(format!("ws-close-{i}")))
                            .ghost()
                            .compact()
                            .icon(IconName::Close)
                            .on_click({
                                let app_state = self.app_state.clone();
                                cx.listener(move |_sidebar, _event, window, cx| {
                                    app_state.update(cx, |state, cx| {
                                        state.close_workspace(i, window, cx);
                                    });
                                })
                            }),
                    );
                }

                row
            };

            tab = tab.child(content);

            if let Some(ref branch) = git_branch {
                tab = tab.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(theme::TEXT_DIM))
                        .overflow_hidden()
                        .child(branch.clone()),
                );
            }

            let tab_with_ctx = tab.context_menu({
                let ws_count = ws_count;
                let sidebar = cx.entity().clone();
                let app_state = self.app_state.clone();
                let name_for_rename = name.clone();
                move |menu, _window, _cx| {
                    let mut menu = menu
                        .item(PopupMenuItem::new("Rename").on_click({
                            let sidebar = sidebar.clone();
                            let name_for_rename = name_for_rename.clone();
                            move |_event, window, cx| {
                                sidebar.update(cx, |sidebar, cx| {
                                    sidebar.start_rename(i, &name_for_rename, window, cx);
                                });
                            }
                        }))
                        .item(PopupMenuItem::new("Duplicate").action(Box::new(DuplicateWorkspace)))
                        .separator()
                        .item(PopupMenuItem::new("Close").on_click({
                            let app_state = app_state.clone();
                            move |_event, window, cx| {
                                app_state.update(cx, |state, cx| {
                                    state.close_workspace(i, window, cx);
                                });
                            }
                        }));
                    if ws_count > 1 {
                        menu = menu.item(PopupMenuItem::new("Close Others").on_click({
                            let app_state = app_state.clone();
                            move |_event, window, cx| {
                                app_state.update(cx, |state, cx| {
                                    let mut indices: Vec<usize> = (0..state.workspaces.len())
                                        .filter(|idx| *idx != i)
                                        .collect();
                                    indices.sort_unstable_by(|a, b| b.cmp(a));
                                    for idx in indices {
                                        state.close_workspace(idx, window, cx);
                                    }
                                    state.set_active_workspace(0, window, cx);
                                });
                            }
                        }));
                    }
                    menu
                }
            });

            tabs = tabs.child(tab_with_ctx);
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
                            .gap(px(2.0))
                            .child(
                                Button::new("sidebar-btn-2")
                                    .ghost()
                                    .compact()
                                    .icon(IconName::Bell)
                                    .tooltip("Notifications")
                                    .on_click(cx.listener(|_s, _e, window, cx| {
                                        window
                                            .dispatch_action(Box::new(ToggleNotificationPanel), cx);
                                    })),
                            )
                            .child(
                                Button::new("sidebar-btn-3")
                                    .ghost()
                                    .compact()
                                    .icon(IconName::Search)
                                    .tooltip("Command Palette")
                                    .on_click(cx.listener(|_s, _e, window, cx| {
                                        window.dispatch_action(Box::new(ToggleCommandPalette), cx);
                                    })),
                            )
                            .child(
                                Button::new("sidebar-btn-4")
                                    .ghost()
                                    .compact()
                                    .icon(IconName::PanelLeftClose)
                                    .tooltip("Toggle Sidebar")
                                    .on_click(cx.listener(|_s, _e, window, cx| {
                                        window.dispatch_action(Box::new(ToggleSidebar), cx);
                                    })),
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
                        Button::new("sidebar-btn-5")
                            .ghost()
                            .icon(IconName::Plus)
                            .label("New Workspace")
                            .on_click({
                                let app_state = self.app_state.clone();
                                cx.listener(move |_sidebar, _event, window, cx| {
                                    app_state.update(cx, |state, cx| {
                                        state.add_workspace(window, cx);
                                    });
                                })
                            }),
                    ),
            )
    }
}
