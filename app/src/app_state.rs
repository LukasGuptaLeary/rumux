use gpui::*;

use crate::config::RumuxConfig;
use crate::notifications::Notification;
use crate::session;
use crate::theme;
use crate::workspace::{Workspace, WorkspaceEvent, WorkspaceSummary};

pub struct AppState {
    pub workspaces: Vec<Entity<Workspace>>,
    pub active_workspace_idx: usize,
    pub config: RumuxConfig,
    pub notifications: Vec<Notification>,
    pub default_cwd: std::path::PathBuf,
    workspace_subscriptions: Vec<Subscription>,
}

impl AppState {
    pub fn new(_cx: &mut App) -> Self {
        let config = RumuxConfig::load();
        let default_cwd = std::env::current_dir()
            .unwrap_or_else(|_| dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/")));

        Self {
            workspaces: Vec::new(),
            active_workspace_idx: 0,
            config,
            notifications: Vec::new(),
            default_cwd,
            workspace_subscriptions: Vec::new(),
        }
    }

    pub fn init_workspaces(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Ok(Some(session_data)) = session::load_session() {
            for ws_session in session_data.workspaces {
                let ws = self.restore_workspace(ws_session, window, cx);
                self.workspaces.push(ws);
            }
            self.active_workspace_idx = session_data
                .active_workspace_idx
                .min(self.workspaces.len().saturating_sub(1));
        }

        if self.workspaces.is_empty() {
            let ws = self.new_workspace_entity("Workspace 1".to_string(), window, cx);
            self.workspaces.push(ws);
        }

        cx.notify();
        self.save_session(cx);
        self.focus_active_workspace(window, cx);
    }

    pub fn active_workspace(&self) -> &Entity<Workspace> {
        &self.workspaces[self.active_workspace_idx]
    }

    pub fn set_active_workspace(
        &mut self,
        idx: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if idx < self.workspaces.len() {
            self.active_workspace_idx = idx;
            cx.notify();
            self.save_session(cx);
            self.focus_active_workspace(window, cx);
        }
    }

    pub fn add_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = format!("Workspace {}", self.workspaces.len() + 1);
        let ws = self.new_workspace_entity(name, window, cx);
        self.workspaces.push(ws);
        self.active_workspace_idx = self.workspaces.len() - 1;
        cx.notify();
        self.save_session(cx);
        self.focus_active_workspace(window, cx);
    }

    pub fn close_workspace(&mut self, idx: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.workspaces.len() <= 1 {
            return;
        }

        self.workspaces.remove(idx);
        if self.active_workspace_idx >= self.workspaces.len() {
            self.active_workspace_idx = self.workspaces.len() - 1;
        }

        cx.notify();
        self.save_session(cx);
        self.focus_active_workspace(window, cx);
    }

    pub fn duplicate_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut snapshot = {
            let current = self.active_workspace().read(cx);
            current.to_session(cx)
        };
        snapshot.name = format!("{} (copy)", snapshot.name);

        let ws = self.restore_workspace(snapshot, window, cx);
        self.workspaces.push(ws);
        self.active_workspace_idx = self.workspaces.len() - 1;

        cx.notify();
        self.save_session(cx);
        self.focus_active_workspace(window, cx);
    }

    pub fn rename_workspace(&mut self, idx: usize, name: String, cx: &mut Context<Self>) {
        if idx >= self.workspaces.len() || name.trim().is_empty() {
            return;
        }

        self.workspaces[idx].update(cx, |ws, cx| {
            ws.name = name;
            cx.notify();
        });

        self.save_session(cx);
    }

    pub fn save_session(&self, cx: &App) {
        let workspaces = self
            .workspaces
            .iter()
            .map(|ws| ws.read(cx).to_session(cx))
            .collect();
        let data = session::SessionData::new(workspaces, self.active_workspace_idx);
        let _ = session::save_session(&data);
    }

    pub fn workspace_summaries(&self, cx: &App) -> Vec<WorkspaceSummary> {
        self.workspaces
            .iter()
            .enumerate()
            .map(|(index, workspace)| {
                workspace
                    .read(cx)
                    .summary(index, index == self.active_workspace_idx, cx)
            })
            .collect()
    }

    pub fn workspace_index_by_name(&self, name: &str, cx: &App) -> Option<usize> {
        self.workspaces
            .iter()
            .position(|workspace| workspace.read(cx).name == name)
    }

    pub fn add_workspace_named(
        &mut self,
        name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| format!("Workspace {}", self.workspaces.len() + 1));
        let ws = self.new_workspace_entity(name, window, cx);
        self.workspaces.push(ws);
        self.active_workspace_idx = self.workspaces.len() - 1;
        cx.notify();
        self.save_session(cx);
        self.focus_active_workspace(window, cx);
    }

    pub fn create_notification(
        &mut self,
        title: String,
        subtitle: Option<String>,
        body: String,
        workspace_id: usize,
        cx: &mut Context<Self>,
    ) -> Notification {
        let notification = Notification {
            id: uuid::Uuid::new_v4().to_string(),
            workspace_id,
            title,
            subtitle,
            body,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or_default(),
            read: false,
        };

        self.notifications.push(notification.clone());

        if let Some(workspace) = self.workspaces.get(workspace_id) {
            workspace.update(cx, |workspace, cx| {
                workspace.unread_count += 1;
                cx.notify();
            });
        }

        cx.notify();
        notification
    }

    pub fn clear_notifications(&mut self, cx: &mut Context<Self>) {
        self.notifications.clear();
        for workspace in &self.workspaces {
            workspace.update(cx, |workspace, cx| {
                workspace.unread_count = 0;
                cx.notify();
            });
        }
        cx.notify();
    }

    #[allow(dead_code)]
    pub fn write_to_target_terminal(&self, text: &str, cx: &mut Context<Self>) {
        let workspace = self.active_workspace().clone();
        workspace.update(cx, |workspace, cx| {
            workspace.write_to_target_terminal(text, &mut **cx);
        });
    }

    fn new_workspace_entity(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<Workspace> {
        let default_cwd = self.default_workspace_cwd();
        let color = Self::workspace_color(self.workspaces.len());
        let ws = cx.new(|cx| {
            let mut workspace = Workspace::new(name, Some(default_cwd), window, cx);
            workspace.color = Some(color);
            workspace
        });
        self.track_workspace(&ws, cx);
        ws
    }

    fn restore_workspace(
        &mut self,
        session: session::WorkspaceSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<Workspace> {
        let default_cwd = self.default_workspace_cwd();
        let color = Self::workspace_color(self.workspaces.len());
        let ws = cx.new(|cx| {
            let mut workspace = Workspace::from_session(session, Some(default_cwd), window, cx);
            workspace.color = Some(color);
            workspace
        });
        self.track_workspace(&ws, cx);
        ws
    }

    fn track_workspace(&mut self, workspace: &Entity<Workspace>, cx: &mut Context<Self>) {
        let subscription = cx.subscribe(
            workspace,
            |state, _workspace, event: &WorkspaceEvent, cx| match event {
                WorkspaceEvent::PersistRequested => state.save_session(cx),
            },
        );
        self.workspace_subscriptions.push(subscription);
    }

    fn default_workspace_cwd(&self) -> String {
        self.default_cwd.to_string_lossy().to_string()
    }

    fn workspace_color(index: usize) -> u32 {
        theme::WORKSPACE_COLORS[index % theme::WORKSPACE_COLORS.len()]
    }

    fn focus_active_workspace(&self, window: &mut Window, cx: &mut Context<Self>) {
        let workspace = self.active_workspace().clone();
        workspace.update(cx, |workspace, cx| {
            workspace.focus_target(window, &mut **cx);
        });
    }
}
