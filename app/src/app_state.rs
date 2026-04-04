use gpui::*;

use crate::pane::Pane;
use crate::terminal_surface::spawn_terminal_view;
use crate::workspace::{SplitDirection, Workspace};

pub struct AppState {
    pub workspaces: Vec<Entity<Workspace>>,
    pub active_workspace_idx: usize,
}

impl AppState {
    pub fn new(cx: &mut App) -> Self {
        let mut state = Self {
            workspaces: Vec::new(),
            active_workspace_idx: 0,
        };
        state.add_workspace_inner(cx);
        state
    }

    fn add_workspace_inner(&mut self, cx: &mut App) {
        let name = format!("Workspace {}", self.workspaces.len() + 1);
        if let Ok(term) = spawn_terminal_view(cx, None, None) {
            let pane = cx.new(|cx| Pane::new(term, cx));
            let ws = cx.new(|_cx| Workspace::new(name, pane));
            self.active_workspace_idx = self.workspaces.len();
            self.workspaces.push(ws);
        }
    }

    pub fn add_workspace(&mut self, cx: &mut Context<Self>) {
        self.add_workspace_inner(&mut **cx);
        cx.notify();
    }

    pub fn close_workspace(&mut self, idx: usize, cx: &mut Context<Self>) {
        if self.workspaces.len() <= 1 {
            return;
        }
        self.workspaces.remove(idx);
        if self.active_workspace_idx >= self.workspaces.len() {
            self.active_workspace_idx = self.workspaces.len() - 1;
        }
        cx.notify();
    }

    pub fn set_active_workspace(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.workspaces.len() {
            self.active_workspace_idx = idx;
            cx.notify();
        }
    }

    pub fn active_workspace(&self) -> &Entity<Workspace> {
        &self.workspaces[self.active_workspace_idx]
    }

    pub fn split_active(&mut self, direction: SplitDirection, cx: &mut Context<Self>) {
        let ws = self.active_workspace().clone();
        ws.update(cx, |ws, cx| {
            ws.split(direction, &mut **cx);
            cx.notify();
        });
        cx.notify();
    }

    pub fn add_terminal_to_active(&mut self, cx: &mut Context<Self>) {
        let ws = self.active_workspace().clone();
        let focused = ws.read(cx).focused_pane.clone();
        focused.update(cx, |pane, cx| {
            pane.add_terminal(&mut **cx);
            cx.notify();
        });
    }

    pub fn close_active_terminal(&mut self, cx: &mut Context<Self>) {
        let ws = self.active_workspace().clone();
        let focused = ws.read(cx).focused_pane.clone();
        let should_remove_pane = focused.update(cx, |pane, cx| {
            let r = pane.close_active_terminal();
            cx.notify();
            r
        });

        if should_remove_pane {
            ws.update(cx, |ws, cx| {
                if let Some(new_layout) =
                    crate::workspace::remove_pane_from_layout(
                        std::mem::replace(
                            &mut ws.layout,
                            crate::workspace::LayoutNode::Leaf(ws.focused_pane.clone()),
                        ),
                        &focused,
                    )
                {
                    ws.layout = new_layout;
                    let panes = ws.panes();
                    if let Some(first) = panes.first() {
                        ws.focused_pane = first.clone();
                    }
                    cx.notify();
                }
            });
        }
    }
}
