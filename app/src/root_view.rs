use gpui::*;

use gpui_component::IconName;
use gpui_component::button::{Button, ButtonVariants};

use crate::app_state::AppState;
use crate::command_palette::{CommandPalette, PaletteEvent};
use crate::find_bar::FindBar;
use crate::notification_panel::NotificationPanel;
use crate::sidebar::WorkspaceSidebar;
use crate::ssh_dialog::{DismissSshDialog, SshDialog};
use crate::theme;

actions!(
    rumux,
    [
        NewWorkspace,
        ConnectSsh,
        CloseWorkspace,
        SplitRight,
        SplitDown,
        NewTerminal,
        NewBrowser,
        CloseTerminal,
        NextWorkspace,
        PrevWorkspace,
        NextTerminal,
        PrevTerminal,
        ToggleCommandPalette,
        ToggleNotificationPanel,
        ToggleSidebar,
        TogglePaneZoom,
        JumpToUnread,
        DuplicateWorkspace,
        ToggleFindBar,
        QuitApp,
    ]
);

pub struct RootView {
    app_state: Entity<AppState>,
    sidebar: Entity<WorkspaceSidebar>,
    command_palette: Option<Entity<CommandPalette>>,
    ssh_dialog: Option<Entity<SshDialog>>,
    notification_panel: Option<Entity<NotificationPanel>>,
    find_bar: Option<Entity<FindBar>>,
    sidebar_visible: bool,
    find_workspace: Option<EntityId>,
    _state_subscription: Subscription,
    pub focus_handle: FocusHandle,
}

impl RootView {
    pub fn new(app_state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let sidebar_visible = app_state.read(cx).config.sidebar_visible;
        let sidebar = cx.new(|cx| WorkspaceSidebar::new(app_state.clone(), cx));

        // Initialize workspaces now that we have window access
        app_state.update(cx, |state, cx| {
            state.init_workspaces(window, cx);
        });

        let state_subscription = cx.observe(&app_state, |root: &mut Self, state, cx| {
            let current = state.read(cx).active_workspace().entity_id();
            if root.find_workspace.is_some_and(|id| id != current) {
                if let Some(bar) = root.find_bar.take() {
                    bar.update(cx, |bar, cx| bar.clear(cx));
                }
                root.find_workspace = None;
            }
            cx.notify();
        });

        Self {
            app_state,
            sidebar,
            command_palette: None,
            ssh_dialog: None,
            notification_panel: None,
            find_bar: None,
            sidebar_visible,
            find_workspace: None,
            _state_subscription: state_subscription,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_ws = self.app_state.read(cx).active_workspace().clone();
        let workspaces = self.app_state.read(cx).workspaces.clone();
        let browser_visible = self.command_palette.is_none()
            && self.ssh_dialog.is_none()
            && self.notification_panel.is_none()
            && self.find_bar.is_none();
        for workspace in workspaces {
            let visible = browser_visible && workspace.entity_id() == active_ws.entity_id();
            workspace.update(cx, |workspace, cx| {
                workspace.set_browsers_visible(visible, cx)
            });
        }

        let mut container = div()
            .size_full()
            .flex()
            .flex_row()
            .bg(rgb(theme::BG_PRIMARY))
            .text_color(rgb(theme::TEXT_PRIMARY))
            .text_size(px(13.0))
            .track_focus(&self.focus_handle)
            // Workspace actions
            .on_action(cx.listener(|root, _: &NewWorkspace, window, cx| {
                root.app_state
                    .update(cx, |state, cx| state.add_workspace(window, cx));
            }))
            .on_action(cx.listener(|root, _: &ConnectSsh, window, cx| {
                let dialog = cx.new(|cx| SshDialog::new(root.app_state.clone(), window, cx));
                cx.subscribe(&dialog, |root, _, _: &DismissSshDialog, cx| {
                    root.ssh_dialog = None;
                    cx.notify();
                })
                .detach();
                root.ssh_dialog = Some(dialog);
                cx.notify();
            }))
            .on_action(cx.listener(|root, _: &CloseWorkspace, _window, cx| {
                let idx = root.app_state.read(cx).active_workspace_idx;
                root.app_state
                    .update(cx, |state, cx| state.close_workspace(idx, _window, cx));
            }))
            .on_action(cx.listener(|root, _: &SplitRight, window, cx| {
                let ws = root.app_state.read(cx).active_workspace().clone();
                ws.update(cx, |ws, cx| {
                    ws.split(gpui_component::Placement::Right, window, cx);
                });
            }))
            .on_action(cx.listener(|root, _: &SplitDown, window, cx| {
                let ws = root.app_state.read(cx).active_workspace().clone();
                ws.update(cx, |ws, cx| {
                    ws.split(gpui_component::Placement::Bottom, window, cx);
                });
            }))
            .on_action(cx.listener(|root, _: &NewTerminal, window, cx| {
                let ws = root.app_state.read(cx).active_workspace().clone();
                ws.update(cx, |ws, cx| {
                    ws.add_terminal(window, cx);
                });
            }))
            .on_action(cx.listener(|root, _: &NewBrowser, window, cx| {
                let workspace = root.app_state.read(cx).active_workspace().clone();
                workspace.update(cx, |workspace, cx| {
                    workspace.add_browser(
                        "about:blank".into(),
                        Some(gpui_component::Placement::Right),
                        window,
                        cx,
                    );
                });
            }))
            .on_action(cx.listener(|root, _: &CloseTerminal, window, cx| {
                let ws = root.app_state.read(cx).active_workspace().clone();
                ws.update(cx, |ws, cx| {
                    ws.close_active_terminal(window, cx);
                });
            }))
            .on_action(cx.listener(|root, _: &NextTerminal, window, cx| {
                let ws = root.app_state.read(cx).active_workspace().clone();
                ws.update(cx, |ws, cx| {
                    ws.next_terminal(window, cx);
                });
            }))
            .on_action(cx.listener(|root, _: &PrevTerminal, window, cx| {
                let ws = root.app_state.read(cx).active_workspace().clone();
                ws.update(cx, |ws, cx| {
                    ws.prev_terminal(window, cx);
                });
            }))
            .on_action(cx.listener(|root, _: &TogglePaneZoom, window, cx| {
                let ws = root.app_state.read(cx).active_workspace().clone();
                ws.update(cx, |ws, cx| {
                    ws.toggle_zoom(window, cx);
                });
            }))
            .on_action(cx.listener(|root, _: &NextWorkspace, window, cx| {
                root.app_state.update(cx, |state, cx| {
                    let next = (state.active_workspace_idx + 1) % state.workspaces.len();
                    state.set_active_workspace(next, window, cx);
                });
            }))
            .on_action(cx.listener(|root, _: &PrevWorkspace, window, cx| {
                root.app_state.update(cx, |state, cx| {
                    let prev = if state.active_workspace_idx == 0 {
                        state.workspaces.len() - 1
                    } else {
                        state.active_workspace_idx - 1
                    };
                    state.set_active_workspace(prev, window, cx);
                });
            }))
            .on_action(cx.listener(|root, _: &DuplicateWorkspace, window, cx| {
                root.app_state
                    .update(cx, |state, cx| state.duplicate_workspace(window, cx));
            }))
            // Quit
            .on_action(cx.listener(|root, _: &QuitApp, _window, cx| {
                root.app_state.read(cx).save_session(cx);
                cx.quit();
            }))
            // Toggle panels
            .on_action(cx.listener(|root, _: &ToggleCommandPalette, window, cx| {
                if root.command_palette.is_some() {
                    root.command_palette = None;
                    root.focus_handle.focus(window);
                } else {
                    let palette = cx.new(|cx| CommandPalette::new(window, cx));
                    cx.subscribe(
                        &palette,
                        |root, _palette, event: &PaletteEvent, cx| match event {
                            PaletteEvent::Dismiss => {
                                root.command_palette = None;
                                cx.notify();
                            }
                        },
                    )
                    .detach();
                    palette.read(cx).focus_handle.focus(window);
                    root.command_palette = Some(palette);
                }
                cx.notify();
            }))
            .on_action(
                cx.listener(|root, _: &ToggleNotificationPanel, window, cx| {
                    if root.notification_panel.is_some() {
                        root.notification_panel = None;
                        root.focus_handle.focus(window);
                    } else {
                        let panel = cx.new(|cx| NotificationPanel::new(root.app_state.clone(), cx));
                        panel.read(cx).focus_handle.focus(window);
                        root.notification_panel = Some(panel);
                    }
                    cx.notify();
                }),
            )
            .on_action(cx.listener(|root, _: &ToggleSidebar, _window, cx| {
                root.sidebar_visible = !root.sidebar_visible;
                cx.notify();
            }))
            .on_action(cx.listener(|root, _: &JumpToUnread, window, cx| {
                root.app_state
                    .update(cx, |state, cx| state.jump_to_unread(window, cx));
            }))
            .on_action(cx.listener(|root, _: &ToggleFindBar, window, cx| {
                if root.find_bar.is_some() {
                    if let Some(bar) = root.find_bar.take() {
                        bar.update(cx, |bar, cx| bar.clear(cx));
                    }
                    root.find_workspace = None;
                    root.app_state
                        .update(cx, |state, cx| state.focus_active_workspace(window, cx));
                } else {
                    let terminal = root
                        .app_state
                        .read(cx)
                        .active_workspace()
                        .read(cx)
                        .target_terminal(cx);
                    if let Some(terminal) = terminal {
                        let bar = cx.new(|cx| FindBar::new(terminal, window, cx));
                        root.find_bar = Some(bar);
                        root.find_workspace =
                            Some(root.app_state.read(cx).active_workspace().entity_id());
                    }
                }
                cx.notify();
            }));

        // Sidebar or expand button
        if self.sidebar_visible {
            container = container.child(self.sidebar.clone());
        } else {
            container = container.child(
                div()
                    .w(px(28.0))
                    .h_full()
                    .flex_shrink_0()
                    .bg(rgb(theme::BG_SECONDARY))
                    .border_r_1()
                    .border_color(rgb(theme::BORDER))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Button::new("root-btn-1")
                            .ghost()
                            .compact()
                            .icon(IconName::PanelLeftOpen)
                            .on_click(cx.listener(|root, _event, _window, cx| {
                                root.sidebar_visible = true;
                                cx.notify();
                            })),
                    ),
            );
        }

        // Main content — active workspace's DockArea
        container = container.child(div().flex_1().overflow_hidden().child(active_ws));

        // Overlays
        if let Some(panel) = &self.notification_panel {
            container = container.child(panel.clone());
        }
        if let Some(palette) = &self.command_palette {
            container = container.child(palette.clone());
        }
        if let Some(bar) = &self.find_bar {
            container = container.child(bar.clone());
        }
        if let Some(dialog) = &self.ssh_dialog {
            container = container.child(dialog.clone());
        }

        container
    }
}
