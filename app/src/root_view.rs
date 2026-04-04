use gpui::*;

use crate::app_state::AppState;
use crate::command_palette::CommandPalette;
use crate::notification_panel::NotificationPanel;
use crate::sidebar::Sidebar;
use crate::theme;
use crate::workspace::SplitDirection;

actions!(
    rumux,
    [
        NewWorkspace,
        CloseWorkspace,
        SplitRight,
        SplitDown,
        NewTerminal,
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
        QuitApp,
    ]
);

pub struct RootView {
    app_state: Entity<AppState>,
    sidebar: Entity<Sidebar>,
    command_palette: Option<Entity<CommandPalette>>,
    notification_panel: Option<Entity<NotificationPanel>>,
    sidebar_visible: bool,
    pub focus_handle: FocusHandle,
}

impl RootView {
    pub fn new(app_state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let sidebar_visible = app_state.read(cx).config.sidebar_visible;
        let sidebar = cx.new(|_cx| Sidebar::new(app_state.clone()));
        Self {
            app_state,
            sidebar,
            command_palette: None,
            notification_panel: None,
            sidebar_visible,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_ws = self.app_state.read(cx).active_workspace().clone();

        let mut container = div()
            .size_full()
            .flex()
            .flex_row()
            .bg(rgb(theme::BG_PRIMARY))
            .text_color(rgb(theme::TEXT_PRIMARY))
            .text_size(px(13.0))
            .track_focus(&self.focus_handle)
            // Workspace actions
            .on_action(cx.listener(|root, _: &NewWorkspace, _window, cx| {
                root.app_state.update(cx, |state, cx| state.add_workspace(cx));
            }))
            .on_action(cx.listener(|root, _: &CloseWorkspace, _window, cx| {
                let idx = root.app_state.read(cx).active_workspace_idx;
                root.app_state
                    .update(cx, |state, cx| state.close_workspace(idx, cx));
            }))
            .on_action(cx.listener(|root, _: &SplitRight, _window, cx| {
                root.app_state
                    .update(cx, |state, cx| state.split_active(SplitDirection::Horizontal, cx));
            }))
            .on_action(cx.listener(|root, _: &SplitDown, _window, cx| {
                root.app_state
                    .update(cx, |state, cx| state.split_active(SplitDirection::Vertical, cx));
            }))
            .on_action(cx.listener(|root, _: &NewTerminal, _window, cx| {
                root.app_state
                    .update(cx, |state, cx| state.add_terminal_to_active(cx));
            }))
            .on_action(cx.listener(|root, _: &CloseTerminal, _window, cx| {
                root.app_state
                    .update(cx, |state, cx| state.close_active_terminal(cx));
            }))
            .on_action(cx.listener(|root, _: &NextWorkspace, _window, cx| {
                root.app_state.update(cx, |state, cx| {
                    let next = (state.active_workspace_idx + 1) % state.workspaces.len();
                    state.set_active_workspace(next, cx);
                });
            }))
            .on_action(cx.listener(|root, _: &PrevWorkspace, _window, cx| {
                root.app_state.update(cx, |state, cx| {
                    let prev = if state.active_workspace_idx == 0 {
                        state.workspaces.len() - 1
                    } else {
                        state.active_workspace_idx - 1
                    };
                    state.set_active_workspace(prev, cx);
                });
            }))
            .on_action(cx.listener(|root, _: &NextTerminal, _window, cx| {
                let ws = root.app_state.read(cx).active_workspace().clone();
                let focused = ws.read(cx).focused_pane.clone();
                focused.update(cx, |pane, _cx| pane.next_terminal());
            }))
            .on_action(cx.listener(|root, _: &PrevTerminal, _window, cx| {
                let ws = root.app_state.read(cx).active_workspace().clone();
                let focused = ws.read(cx).focused_pane.clone();
                focused.update(cx, |pane, _cx| pane.prev_terminal());
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
                    let palette = cx.new(|cx| CommandPalette::new(cx));
                    palette.read(cx).focus_handle.focus(window);
                    root.command_palette = Some(palette);
                }
                cx.notify();
            }))
            .on_action(cx.listener(|root, _: &ToggleNotificationPanel, window, cx| {
                if root.notification_panel.is_some() {
                    root.notification_panel = None;
                    root.focus_handle.focus(window);
                } else {
                    let panel =
                        cx.new(|cx| NotificationPanel::new(root.app_state.clone(), cx));
                    panel.read(cx).focus_handle.focus(window);
                    root.notification_panel = Some(panel);
                }
                cx.notify();
            }))
            .on_action(cx.listener(|root, _: &ToggleSidebar, _window, cx| {
                root.sidebar_visible = !root.sidebar_visible;
                cx.notify();
            }))
            .on_action(cx.listener(|root, _: &TogglePaneZoom, _window, cx| {
                let ws = root.app_state.read(cx).active_workspace().clone();
                ws.update(cx, |ws, cx| {
                    ws.toggle_zoom();
                    cx.notify();
                });
                cx.notify();
            }))
            .on_action(cx.listener(|root, _: &JumpToUnread, _window, cx| {
                let state = root.app_state.read(cx);
                for (i, ws) in state.workspaces.iter().enumerate() {
                    if ws.read(cx).unread_count > 0 {
                        drop(state);
                        root.app_state
                            .update(cx, |state, cx| state.set_active_workspace(i, cx));
                        break;
                    }
                }
            }));

        // Sidebar
        if self.sidebar_visible {
            container = container.child(self.sidebar.clone());
        }

        // Main content
        container = container.child(div().flex_1().overflow_hidden().child(active_ws));

        // Overlays
        if let Some(panel) = &self.notification_panel {
            container = container.child(panel.clone());
        }
        if let Some(palette) = &self.command_palette {
            container = container.child(palette.clone());
        }

        container
    }
}
