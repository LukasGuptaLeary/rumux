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
    primary_window: Option<AnyWindowHandle>,
    workspace_subscriptions: Vec<Subscription>,
    session_writable: bool,
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
            primary_window: None,
            workspace_subscriptions: Vec::new(),
            session_writable: true,
        }
    }

    pub fn init_workspaces(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let loaded = match session::load_session() {
            Ok(data) => data,
            Err(error) => {
                eprintln!("Unable to restore session: {error:#}. Session file will be preserved.");
                self.session_writable = false;
                None
            }
        };
        if let Some(session_data) = loaded {
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
        if self.workspaces.len() <= 1 || idx >= self.workspaces.len() {
            return;
        }

        for surface in self.workspaces[idx].read(cx).list_surfaces(cx) {
            if surface.kind == "terminal"
                && let Ok(id) = uuid::Uuid::parse_str(&surface.id)
            {
                crate::terminal_surface::close_owned(id, cx);
            }
        }
        self.workspaces.remove(idx);
        self.notifications
            .retain(|notification| notification.workspace_id != idx);
        for notification in &mut self.notifications {
            if notification.workspace_id > idx {
                notification.workspace_id -= 1;
            }
        }
        if self.active_workspace_idx > idx {
            self.active_workspace_idx -= 1;
        }
        if self.active_workspace_idx >= self.workspaces.len() {
            self.active_workspace_idx = self.workspaces.len() - 1;
        }

        cx.notify();
        self.save_session(cx);
        self.focus_active_workspace(window, cx);
    }

    pub fn duplicate_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let idx = self.active_workspace_idx;
        let _ = self.duplicate_workspace_at(idx, window, cx);
    }

    pub fn duplicate_workspace_at(
        &mut self,
        idx: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        if idx >= self.workspaces.len() {
            return None;
        }

        let mut snapshot = {
            let workspace = self.workspaces[idx].read(cx);
            workspace.to_session(cx)
        };
        snapshot.name = format!("{} (copy)", snapshot.name);
        snapshot.id = None;
        if let Some(connection) = &mut snapshot.ssh {
            connection.forwards.clear();
        }
        if let Some(dock) = &mut snapshot.dock_area {
            clear_panel_ids(&mut dock.center);
        }

        let ws = self.restore_workspace(snapshot, window, cx);
        self.workspaces.push(ws);
        self.active_workspace_idx = self.workspaces.len() - 1;

        cx.notify();
        self.save_session(cx);
        self.focus_active_workspace(window, cx);
        Some(self.active_workspace_idx)
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
        if !self.session_writable {
            return;
        }
        let workspaces = self
            .workspaces
            .iter()
            .map(|ws| ws.read(cx).to_session(cx))
            .collect();
        let data = session::SessionData::new(workspaces, self.active_workspace_idx);
        if let Err(error) = session::save_session(&data) {
            eprintln!("Unable to save session: {error:#}");
        }
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

    pub fn add_ssh_workspace(
        &mut self,
        name: Option<String>,
        connection: rumux_core::ssh::SshConnection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| format!("SSH {}", connection.destination()));
        let color = Self::workspace_color(self.workspaces.len());
        let workspace = cx.new(|cx| {
            let mut workspace = Workspace::new_ssh(name, connection, window, cx);
            workspace.color = Some(color);
            workspace
        });
        self.track_workspace(&workspace, cx);
        self.workspaces.push(workspace);
        self.active_workspace_idx = self.workspaces.len() - 1;
        self.save_session(cx);
        cx.notify();
        self.focus_active_workspace(window, cx);
    }

    pub fn create_notification_at(
        &mut self,
        title: String,
        subtitle: Option<String>,
        body: String,
        workspace_id: usize,
        surface_index: Option<usize>,
        cx: &mut Context<Self>,
    ) -> Notification {
        let notification = Notification {
            id: uuid::Uuid::new_v4().to_string(),
            workspace_id,
            surface_index,
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
        if self.notifications.len() > 1_000 {
            self.notifications.remove(0);
        }
        self.refresh_unread_counts(cx);
        if self.config.desktop_notifications {
            let title = notification.title.clone();
            let body = notification.body.clone();
            cx.background_executor()
                .spawn(async move {
                    let _ = notify_rust::Notification::new()
                        .summary(&title)
                        .body(&body)
                        .appname("rumux")
                        .show();
                })
                .detach();
        }

        cx.notify();
        notification
    }

    pub fn clear_notifications(&mut self, cx: &mut Context<Self>) {
        self.notifications.clear();
        self.refresh_unread_counts(cx);
        cx.notify();
    }

    fn refresh_unread_counts(&mut self, cx: &mut Context<Self>) {
        for (index, workspace) in self.workspaces.iter().enumerate() {
            let pending: Vec<_> = self
                .notifications
                .iter()
                .filter(|n| n.workspace_id == index && !n.read)
                .collect();
            workspace.update(cx, |workspace, cx| {
                workspace.unread_count = pending.len();
                workspace.set_unread_surfaces(
                    pending.iter().filter_map(|n| n.surface_index).collect(),
                    cx,
                );
                cx.notify();
            });
        }
    }

    pub fn open_notification(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(notification) = self.notifications.iter_mut().find(|n| n.id == id) else {
            return false;
        };
        notification.read = true;
        let workspace_index = notification.workspace_id;
        let surface_index = notification.surface_index;
        self.refresh_unread_counts(cx);
        if workspace_index >= self.workspaces.len() {
            return false;
        }
        self.set_active_workspace(workspace_index, window, cx);
        self.workspaces[workspace_index]
            .clone()
            .update(cx, |workspace, cx| {
                workspace.focus_surface(surface_index, window, cx);
            });
        cx.notify();
        true
    }

    pub fn jump_to_unread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self
            .notifications
            .iter()
            .rev()
            .find(|n| !n.read)
            .map(|n| n.id.clone())
        {
            self.open_notification(&id, window, cx);
        }
    }

    pub fn set_primary_window(&mut self, window: AnyWindowHandle) {
        self.primary_window = Some(window);
    }

    pub fn primary_window(&self) -> Option<AnyWindowHandle> {
        self.primary_window
    }

    pub fn focus_active_workspace(&self, window: &mut Window, cx: &mut Context<Self>) {
        let workspace = self.active_workspace().clone();
        workspace.update(cx, |workspace, cx| {
            workspace.focus_target(window, &mut **cx);
        });
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
            |state, workspace, event: &WorkspaceEvent, cx| match event {
                WorkspaceEvent::PersistRequested => state.save_session(cx),
                WorkspaceEvent::Notification {
                    surface_index,
                    title,
                    body,
                } => {
                    if let Some(index) = state
                        .workspaces
                        .iter()
                        .position(|ws| ws.entity_id() == workspace.entity_id())
                    {
                        state.create_notification_at(
                            title.clone(),
                            None,
                            body.clone(),
                            index,
                            Some(*surface_index),
                            cx,
                        );
                    }
                }
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
}

fn clear_panel_ids(panel: &mut gpui_component::dock::PanelState) {
    if matches!(panel.panel_name.as_str(), "TerminalPanel" | "BrowserPanel") {
        if let gpui_component::dock::PanelInfo::Panel(value) = &mut panel.info {
            if let Some(info) = value.as_object_mut() {
                info.remove("id");
                if let Some(ssh) = info.get_mut("ssh").filter(|ssh| ssh.is_object()) {
                    ssh["forwards"] = serde_json::json!([]);
                }
            }
        }
    }
    for child in &mut panel.children {
        clear_panel_ids(child);
    }
}
