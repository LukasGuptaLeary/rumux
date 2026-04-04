use gpui::*;

use crate::app_state::AppState;
use crate::command_palette::CommandPalette;
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
    ]
);

pub struct RootView {
    app_state: Entity<AppState>,
    sidebar: Entity<Sidebar>,
    command_palette: Option<Entity<CommandPalette>>,
    pub focus_handle: FocusHandle,
}

impl RootView {
    pub fn new(app_state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let sidebar = cx.new(|_cx| Sidebar::new(app_state.clone()));
        Self {
            app_state,
            sidebar,
            command_palette: None,
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
            .child(self.sidebar.clone())
            .child(div().flex_1().overflow_hidden().child(active_ws));

        // Command palette overlay
        if let Some(palette) = &self.command_palette {
            container = container.child(palette.clone());
        }

        container
    }
}
