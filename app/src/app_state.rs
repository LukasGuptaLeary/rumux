use gpui::*;

use crate::pane::Pane;
use crate::session::{self, SessionData, WorkspaceSession};
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

        // Try to restore session
        if let Ok(Some(session_data)) = session::load_session() {
            for ws_session in &session_data.workspaces {
                let cwd = if ws_session.cwd.is_empty() {
                    None
                } else {
                    Some(std::path::Path::new(&ws_session.cwd))
                };
                if let Ok(term) = spawn_terminal_view(cx, cwd, None) {
                    let pane = cx.new(|cx| Pane::new(term, cx));
                    let ws = cx.new(|_cx| Workspace::new(ws_session.name.clone(), pane));
                    state.workspaces.push(ws);
                }
            }
            if !state.workspaces.is_empty() {
                state.active_workspace_idx =
                    session_data.active_workspace_idx.min(state.workspaces.len() - 1);
            }
        }

        // Ensure at least one workspace
        if state.workspaces.is_empty() {
            state.add_workspace_inner(cx);
        }

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

    pub fn save_session(&self, cx: &App) {
        let workspaces: Vec<WorkspaceSession> = self
            .workspaces
            .iter()
            .map(|ws| {
                let ws = ws.read(cx);
                WorkspaceSession {
                    name: ws.name.clone(),
                    cwd: String::new(), // TODO: track cwd per workspace
                }
            })
            .collect();

        let data = SessionData {
            window_width: 1200.0,
            window_height: 800.0,
            workspaces,
            active_workspace_idx: self.active_workspace_idx,
        };

        if let Err(e) = session::save_session(&data) {
            eprintln!("Failed to save session: {e}");
        }
    }
}
