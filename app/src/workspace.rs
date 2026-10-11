use std::sync::Arc;

use gpui::*;
use gpui_component::Placement;
use gpui_component::dock::{
    DockArea, DockAreaState, DockEvent, DockItem, DockPlacement, PanelInfo, PanelState, PanelStyle,
    PanelView, TabPanel,
};
use serde::Serialize;
use uuid::Uuid;

use crate::browser_panel::BrowserPanel;
use crate::session::WorkspaceSession;
use crate::terminal_panel::{TerminalPanel, TerminalPanelEvent};
use crate::terminal_surface::terminal_error_view;
use crate::theme;
use rumux_core::terminal_protocol::TerminalSignal;

pub enum WorkspaceEvent {
    PersistRequested,
    Notification {
        surface_index: usize,
        title: String,
        body: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct SurfaceSummary {
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh: Option<rumux_core::ssh::SshConnection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub id: String,
    pub index: usize,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    pub active_tab: bool,
    pub target: bool,
    pub exited: bool,
    pub status: rumux_core::status::SurfaceStatus,
    pub status_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh: Option<rumux_core::ssh::SshConnection>,
    pub id: String,
    pub index: usize,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<u32>,
    pub unread_count: usize,
    pub active: bool,
    pub surface_count: usize,
    pub status: rumux_core::status::SurfaceStatus,
}

#[derive(Debug, Clone, Copy)]
pub enum SurfaceReadScope {
    Buffer,
    Visible,
}

#[derive(Clone)]
struct SurfaceTarget {
    tab_panel: Entity<TabPanel>,
    tab_position: usize,
    panel: Entity<TerminalPanel>,
    active_tab: bool,
    target: bool,
}

pub struct Workspace {
    pub ssh: Option<rumux_core::ssh::SshConnection>,
    pub id: Uuid,
    pub name: String,
    pub dock_area: Entity<DockArea>,
    pub unread_count: usize,
    pub color: Option<u32>,
    pub git_branch: Option<String>,
    pub cwd: Option<String>,
    next_terminal_index: usize,
    _dock_subscription: Subscription,
    panel_subscriptions: Vec<(EntityId, Subscription)>,
    owned_terminal_ids: std::collections::HashSet<Uuid>,
}

impl Workspace {
    pub fn new(
        name: String,
        cwd: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(name, cwd, None, None, Some(1), window, cx)
    }

    pub fn new_ssh(
        name: String,
        connection: rumux_core::ssh::SshConnection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(
            name,
            connection.directory.clone(),
            Some(connection),
            None,
            Some(1),
            window,
            cx,
        )
    }

    pub fn from_session(
        session: WorkspaceSession,
        fallback_cwd: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let cwd = if session.cwd.trim().is_empty() {
            fallback_cwd
        } else {
            Some(session.cwd.clone())
        };
        let next_terminal_index = session
            .next_terminal_index
            .unwrap_or_else(|| next_terminal_index_from_state(session.dock_area.as_ref()));

        let id = session
            .id
            .as_deref()
            .and_then(|id| Uuid::parse_str(id).ok());
        let mut workspace = Self::build(
            session.name,
            cwd,
            session.ssh,
            session.dock_area,
            Some(next_terminal_index),
            window,
            cx,
        );
        if let Some(id) = id {
            workspace.id = id;
        }
        workspace
    }

    pub fn to_session(&self, cx: &App) -> WorkspaceSession {
        WorkspaceSession {
            ssh: self.ssh.clone(),
            id: Some(self.id.to_string()),
            name: self.name.clone(),
            cwd: self.cwd.clone().unwrap_or_default(),
            dock_area: Some(self.dock_area.read(cx).dump(cx)),
            next_terminal_index: Some(self.current_next_terminal_index(cx)),
        }
    }

    fn build(
        name: String,
        cwd: Option<String>,
        ssh: Option<rumux_core::ssh::SshConnection>,
        dock_state: Option<DockAreaState>,
        next_terminal_index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let dock_area = cx.new(|cx| {
            DockArea::new(format!("workspace-{}", Uuid::new_v4()), None, window, cx)
                .panel_style(PanelStyle::TabBar)
        });
        let dock_subscription = cx.subscribe(&dock_area, |workspace, _, event: &DockEvent, cx| {
            if matches!(event, DockEvent::LayoutChanged) {
                workspace.track_terminal_panels(cx);
                workspace.next_terminal_index = workspace.current_next_terminal_index(cx);
                cx.emit(WorkspaceEvent::PersistRequested);
            }
        });

        let initial_cwd = cwd.as_deref().map(std::path::Path::new);
        let mut loaded_state = false;

        if let Some(state) = dock_state {
            loaded_state = dock_area
                .update(cx, |area, cx| area.load(state, window, cx))
                .is_ok();
        }

        if !loaded_state {
            let center = Self::default_center(initial_cwd, ssh.as_ref(), &dock_area, window, cx);
            dock_area.update(cx, |area, cx| {
                area.set_center(center, window, cx);
            });
        }

        let mut workspace = Self {
            ssh,
            id: Uuid::new_v4(),
            name,
            dock_area,
            unread_count: 0,
            color: None,
            git_branch: None,
            cwd,
            next_terminal_index: next_terminal_index.unwrap_or(1).max(1),
            _dock_subscription: dock_subscription,
            panel_subscriptions: Vec::new(),
            owned_terminal_ids: std::collections::HashSet::new(),
        };
        workspace.track_terminal_panels(cx);
        workspace
    }

    fn track_terminal_panels(&mut self, cx: &mut Context<Self>) {
        let panels: Vec<_> = self
            .surface_targets(cx)
            .into_iter()
            .map(|target| target.panel)
            .collect();
        let current: std::collections::HashSet<_> =
            panels.iter().map(|panel| panel.read(cx).id).collect();
        for id in self.owned_terminal_ids.difference(&current) {
            crate::terminal_surface::close_owned(*id, cx);
        }
        self.owned_terminal_ids = current;
        self.panel_subscriptions
            .retain(|(id, _)| panels.iter().any(|panel| panel.entity_id() == *id));
        for panel in panels {
            let id = panel.entity_id();
            if self
                .panel_subscriptions
                .iter()
                .any(|(tracked, _)| *tracked == id)
            {
                continue;
            }
            let subscription = cx.subscribe(
                &panel,
                |workspace, panel, event: &TerminalPanelEvent, cx| {
                    match event {
                        TerminalPanelEvent::Signal(TerminalSignal::Notification {
                            title,
                            body,
                        }) => {
                            cx.emit(WorkspaceEvent::Notification {
                                surface_index: panel.read(cx).index(),
                                title: title.clone(),
                                body: body.clone(),
                            });
                        }
                        TerminalPanelEvent::Signal(TerminalSignal::WorkingDirectory(_)) => {
                            if let Some(target) = workspace.resolve_surface_target(None, cx) {
                                workspace.cwd = target.panel.read(cx).cwd().map(str::to_string);
                            }
                            cx.emit(WorkspaceEvent::PersistRequested);
                        }
                        _ => {}
                    }
                    cx.notify();
                },
            );
            self.panel_subscriptions.push((id, subscription));
        }
    }

    pub fn target_terminal(&self, cx: &App) -> Option<Entity<gpui_terminal::TerminalView>> {
        self.resolve_surface_target(None, cx)
            .map(|target| target.panel.read(cx).terminal())
    }

    pub fn set_unread_surfaces(&mut self, indices: Vec<usize>, cx: &mut Context<Self>) {
        for target in self.surface_targets(cx) {
            target.panel.update(cx, |panel, cx| {
                panel.unread = indices.contains(&panel.index());
                cx.notify();
            });
        }
    }

    fn default_center(
        cwd: Option<&std::path::Path>,
        ssh: Option<&rumux_core::ssh::SshConnection>,
        dock_area: &Entity<DockArea>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> DockItem {
        let panel = Self::create_terminal_panel(cwd, ssh, 0, cx);
        DockItem::h_split(
            vec![DockItem::tab(
                panel,
                &dock_area.downgrade(),
                window,
                &mut **cx,
            )],
            &dock_area.downgrade(),
            window,
            &mut **cx,
        )
    }

    fn create_terminal_panel(
        cwd: Option<&std::path::Path>,
        ssh: Option<&rumux_core::ssh::SshConnection>,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Entity<TerminalPanel> {
        cx.new(|cx| {
            let created = if let Some(connection) = ssh {
                TerminalPanel::from_ssh(connection.clone(), index, uuid::Uuid::new_v4(), cx)
            } else {
                TerminalPanel::from_cwd(cwd, index, cx)
            };
            created.unwrap_or_else(|error| {
                let term = terminal_error_view(&mut **cx, &error);
                TerminalPanel::new(
                    term,
                    index,
                    cwd.map(|path| path.to_string_lossy().into_owned()),
                    cx,
                )
            })
        })
    }

    pub fn add_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cwd = self
            .resolve_surface_target(None, cx)
            .and_then(|target| target.panel.read(cx).cwd().map(str::to_string))
            .or_else(|| self.cwd.clone());
        let (_, _, panel_view) =
            self.new_terminal_panel_view(cwd.as_deref().map(std::path::Path::new), cx);

        if let Some(target_tp) = self.target_tab_panel(Some(window), cx) {
            target_tp.update(cx, |tp, cx| {
                tp.add_panel(panel_view, window, cx);
            });
        } else {
            self.dock_area.update(cx, |area, cx| {
                area.add_panel(panel_view, DockPlacement::Center, None, window, cx);
            });
        }

        cx.notify();
    }

    pub fn split(&mut self, placement: Placement, window: &mut Window, cx: &mut Context<Self>) {
        let cwd = self
            .resolve_surface_target(None, cx)
            .and_then(|target| target.panel.read(cx).cwd().map(str::to_string))
            .or_else(|| self.cwd.clone());
        let (_, _, panel_view) =
            self.new_terminal_panel_view(cwd.as_deref().map(std::path::Path::new), cx);

        if let Some(target_tp) = self.target_tab_panel(Some(window), cx) {
            target_tp.update(cx, |tp, cx| {
                tp.add_panel_at(panel_view, placement, None, window, cx);
            });
        } else {
            self.dock_area.update(cx, |area, cx| {
                area.add_panel(panel_view, DockPlacement::Center, None, window, cx);
            });
        }

        cx.notify();
    }

    pub fn close_active_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(target_tp) = self.target_tab_panel(Some(window), cx) {
            let active = target_tp.read(cx).active_panel(cx);
            if let Some(panel) = active {
                target_tp.update(cx, |tp, cx| {
                    tp.remove_panel(panel, window, cx);
                });
            }
        }

        cx.notify();
    }

    pub fn next_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(target_tp) = self.target_tab_panel(Some(window), cx) {
            target_tp.update(cx, |tp, cx| {
                tp.next_tab(window, cx);
            });
        }
    }

    pub fn prev_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(target_tp) = self.target_tab_panel(Some(window), cx) {
            target_tp.update(cx, |tp, cx| {
                tp.prev_tab(window, cx);
            });
        }
    }

    pub fn toggle_zoom(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let is_zoomed = {
            let dock = self.dock_area.read(cx);
            dock.is_zoomed()
        };

        if let Some(target_tp) = self.target_tab_panel(Some(window), cx) {
            if is_zoomed {
                self.dock_area.update(cx, |area, cx| {
                    area.set_zoomed_out(window, cx);
                });
            } else {
                self.dock_area.update(cx, |area, cx| {
                    area.set_zoomed_in(target_tp, window, cx);
                });
            }
        }

        cx.notify();
    }

    pub fn focus_target(&self, window: &mut Window, cx: &mut App) {
        if let Some(tab_panel) = self.target_tab_panel(Some(window), cx) {
            tab_panel.read(cx).focus_handle(cx).focus(window);
        }
    }

    pub fn summary(&self, index: usize, active: bool, cx: &App) -> WorkspaceSummary {
        WorkspaceSummary {
            ssh: self.ssh.clone(),
            id: self.id.to_string(),
            index,
            name: self.name.clone(),
            cwd: self.cwd.clone(),
            git_branch: self.git_branch.clone(),
            color: self.color,
            unread_count: self.unread_count,
            active,
            surface_count: self.list_surfaces(cx).len(),
            status: self.activity_status(cx),
        }
    }

    pub fn list_surfaces(&self, cx: &App) -> Vec<SurfaceSummary> {
        let mut surfaces: Vec<_> = self
            .surface_targets(cx)
            .into_iter()
            .map(|target| {
                let panel = target.panel.read(cx);
                SurfaceSummary {
                    kind: "terminal",
                    ssh: panel.ssh.clone(),
                    url: None,
                    id: panel.id.to_string(),
                    index: panel.index(),
                    title: panel.display_name(),
                    cwd: panel.cwd().map(str::to_string),
                    active_tab: target.active_tab,
                    target: target.target,
                    exited: panel.exited,
                    status: panel.status,
                    status_message: panel.status_message.clone(),
                }
            })
            .collect();
        let target_id = self
            .target_tab_panel(None, cx)
            .and_then(|tab| tab.read(cx).active_panel(cx))
            .map(|panel| panel.view().entity_id());
        for (tab, position, panel) in self.browser_targets(cx) {
            let browser = panel.read(cx);
            surfaces.push(SurfaceSummary {
                id: browser.id.to_string(),
                index: browser.index,
                title: browser.title_text(cx),
                kind: "browser",
                ssh: None,
                url: Some(browser.browser.read(cx).url.clone()),
                cwd: None,
                active_tab: tab.read(cx).active_index() == position,
                target: Some(panel.entity_id()) == target_id,
                exited: false,
                status: rumux_core::status::SurfaceStatus::Idle,
                status_message: None,
            });
        }
        surfaces
    }

    fn browser_targets(&self, cx: &App) -> Vec<(Entity<TabPanel>, usize, Entity<BrowserPanel>)> {
        let mut result = Vec::new();
        for tab in self.dock_area.read(cx).items().all_tab_panels(cx) {
            for (position, panel) in tab.read(cx).panels().iter().enumerate() {
                if panel.panel_name(cx) == "BrowserPanel" {
                    result.push((
                        tab.clone(),
                        position,
                        Entity::<BrowserPanel>::from(panel.as_ref()),
                    ));
                }
            }
        }
        result
    }

    pub fn browser_for_surface(
        &self,
        index: Option<usize>,
        cx: &App,
    ) -> Option<Entity<crate::browser::BrowserView>> {
        let target = self
            .target_tab_panel(None, cx)
            .and_then(|tab| tab.read(cx).active_panel(cx))
            .map(|panel| panel.view().entity_id());
        self.browser_targets(cx)
            .into_iter()
            .find(|(_, _, panel)| {
                index.map_or(Some(panel.entity_id()) == target, |index| {
                    panel.read(cx).index == index
                })
            })
            .map(|(_, _, panel)| panel.read(cx).browser.clone())
    }

    pub fn set_browsers_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        let zoomed = self.dock_area.read(cx).zoomed_view_id();
        for (tab, _, panel) in self.browser_targets(cx) {
            let allowed =
                visible && zoomed.is_none_or(|id| id == tab.entity_id() || id == panel.entity_id());
            panel.update(cx, |panel, cx| panel.set_allowed(allowed, cx));
        }
    }

    pub fn add_browser(
        &mut self,
        url: String,
        placement: Option<Placement>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> usize {
        let index = self.current_next_terminal_index(cx);
        let panel = cx.new(|cx| BrowserPanel::new(index, Uuid::new_v4(), url, window, cx));
        let panel_view: Arc<dyn PanelView> = Arc::new(panel);
        if let Some(tab) = self.target_tab_panel(Some(window), cx) {
            if let Some(placement) = placement {
                if self
                    .insert_split_panel(&tab, panel_view.clone(), placement, window, cx)
                    .is_none()
                {
                    tab.update(cx, |tab, cx| tab.add_panel(panel_view, window, cx));
                }
            } else {
                tab.update(cx, |tab, cx| tab.add_panel(panel_view, window, cx));
            }
        } else {
            self.dock_area.update(cx, |area, cx| {
                area.add_panel(panel_view, DockPlacement::Center, None, window, cx)
            });
        }
        cx.emit(WorkspaceEvent::PersistRequested);
        cx.notify();
        index
    }

    pub fn set_surface_status(
        &mut self,
        surface_index: Option<usize>,
        status: rumux_core::status::SurfaceStatus,
        message: Option<String>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(target) = self.resolve_surface_target(surface_index, cx) else {
            return false;
        };
        let client = cx
            .global::<crate::terminal_service::ServiceClient>()
            .0
            .clone();
        let id = target.panel.read(cx).id;
        let service_message = message.clone();
        std::thread::spawn(move || {
            let _ = client.call(
                "status",
                serde_json::json!({"id":id.to_string(),"status":status,"message":service_message}),
            );
        });
        target.panel.update(cx, |panel, cx| {
            panel.status = if panel.exited {
                rumux_core::status::SurfaceStatus::Exited
            } else {
                status
            };
            panel.status_message = message;
            cx.notify();
        });
        cx.notify();
        true
    }

    pub fn activity_status(&self, cx: &App) -> rumux_core::status::SurfaceStatus {
        use rumux_core::status::SurfaceStatus;
        let surfaces = self.list_surfaces(cx);
        if surfaces
            .iter()
            .any(|surface| surface.status == SurfaceStatus::Blocked)
        {
            SurfaceStatus::Blocked
        } else if surfaces
            .iter()
            .any(|surface| surface.status == SurfaceStatus::Working)
        {
            SurfaceStatus::Working
        } else if !surfaces.is_empty() && surfaces.iter().all(|surface| surface.exited) {
            SurfaceStatus::Exited
        } else {
            SurfaceStatus::Idle
        }
    }

    pub fn focus_surface(
        &mut self,
        surface_index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some((tab, position, panel)) = self
            .browser_targets(cx)
            .into_iter()
            .find(|(_, _, panel)| surface_index == Some(panel.read(cx).index))
        {
            tab.update(cx, |tab, cx| tab.set_active_index(position, window, cx));
            panel.read(cx).focus_handle(cx).focus(window);
            return true;
        }
        let Some(target) = self.resolve_surface_target(surface_index, cx) else {
            return false;
        };

        target.tab_panel.update(cx, |tab_panel, cx| {
            tab_panel.set_active_index(target.tab_position, window, cx);
        });
        target.panel.read(cx).focus_handle(cx).focus(window);
        true
    }

    pub fn send_text_to_surface(
        &self,
        surface_index: Option<usize>,
        text: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(panel) = self
            .resolve_surface_target(surface_index, cx)
            .map(|target| target.panel)
        else {
            return false;
        };

        panel.update(cx, |panel, cx| {
            panel.write_to_terminal(text.as_bytes(), &mut **cx);
        });
        true
    }

    pub fn send_keystroke_to_surface(
        &self,
        surface_index: Option<usize>,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(panel) = self
            .resolve_surface_target(surface_index, cx)
            .map(|target| target.panel)
        else {
            return false;
        };

        panel.update(cx, |panel, cx| panel.send_keystroke(keystroke, &mut **cx))
    }

    #[allow(dead_code)]
    pub fn write_to_target_terminal(&self, text: &str, cx: &mut App) {
        if let Some(panel) = self.target_terminal_panel(cx) {
            panel.update(cx, |panel, cx| {
                panel.write_to_terminal(text.as_bytes(), &mut **cx);
            });
        }
    }

    pub fn add_terminal_to_surface(
        &mut self,
        surface_index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        let target = self.resolve_tab_target(surface_index, cx);
        if surface_index.is_some() && target.is_none() {
            return None;
        }
        let cwd = self
            .resolve_surface_target(surface_index, cx)
            .and_then(|target| target.panel.read(cx).cwd().map(str::to_string))
            .or_else(|| self.cwd.clone());
        let (index, _, panel_view) =
            self.new_terminal_panel_view(cwd.as_deref().map(std::path::Path::new), cx);
        if let Some((tab, position)) = target {
            tab.update(cx, |tab, cx| {
                tab.set_active_index(position, window, cx);
                tab.add_panel(panel_view, window, cx);
            });
        } else {
            self.dock_area.update(cx, |area, cx| {
                area.add_panel(panel_view, DockPlacement::Center, None, window, cx);
            });
        }
        cx.notify();
        Some(index)
    }

    pub fn split_surface(
        &mut self,
        surface_index: Option<usize>,
        placement: Placement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        let Some((tab, _)) = self.resolve_tab_target(surface_index, cx) else {
            return if surface_index.is_none() {
                self.add_terminal_to_surface(None, window, cx)
            } else {
                None
            };
        };
        let cwd = self
            .resolve_surface_target(surface_index, cx)
            .and_then(|target| target.panel.read(cx).cwd().map(str::to_string))
            .or_else(|| self.cwd.clone());
        let (index, _, panel_view) =
            self.new_terminal_panel_view(cwd.as_deref().map(std::path::Path::new), cx);
        self.insert_split_panel(&tab, panel_view, placement, window, cx)?;
        self.track_terminal_panels(cx);
        cx.emit(WorkspaceEvent::PersistRequested);
        cx.notify();
        Some(index)
    }

    // Commit the tree change before replying to RPC so callers can immediately
    // address the created surface by its actual UUID.
    fn insert_split_panel(
        &mut self,
        target: &Entity<TabPanel>,
        panel: Arc<dyn PanelView>,
        placement: Placement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<()> {
        let dock = self.dock_area.downgrade();
        let item = DockItem::tabs(vec![panel], &dock, window, &mut **cx);
        let DockItem::Tabs { view, .. } = &item else {
            return None;
        };
        let new_target = view.downgrade();
        let current = self.dock_area.read(cx).items().clone();
        let split = split_dock_item(
            current,
            target.entity_id(),
            item,
            placement,
            &dock,
            window,
            &mut **cx,
        )?;
        self.dock_area.update(cx, |area, cx| {
            area.set_center(split, window, cx);
            area.remember_tab_panel(new_target);
        });
        Some(())
    }

    pub fn close_surface(
        &mut self,
        surface_index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some((tab, _, panel)) = self
            .browser_targets(cx)
            .into_iter()
            .find(|(_, _, panel)| surface_index == Some(panel.read(cx).index))
        {
            let view: Arc<dyn PanelView> = Arc::new(panel);
            tab.update(cx, |tab, cx| tab.remove_panel(view, window, cx));
            return true;
        }
        let Some(target) = self.resolve_surface_target(surface_index, cx) else {
            return false;
        };

        let panel_view: Arc<dyn PanelView> = Arc::new(target.panel);
        target.tab_panel.update(cx, |tab_panel, cx| {
            tab_panel.remove_panel(panel_view, window, cx);
        });
        cx.notify();
        true
    }

    pub fn rename_surface(
        &mut self,
        surface_index: Option<usize>,
        name: Option<String>,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some((_, _, panel)) = self
            .browser_targets(cx)
            .into_iter()
            .find(|(_, _, panel)| surface_index == Some(panel.read(cx).index))
        {
            panel.update(cx, |panel, cx| {
                panel.name = name;
                cx.notify();
            });
            cx.emit(WorkspaceEvent::PersistRequested);
            return true;
        }
        let Some(panel) = self
            .resolve_surface_target(surface_index, cx)
            .map(|target| target.panel)
        else {
            return false;
        };

        let name = name
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty());
        panel.update(cx, |panel, cx| {
            panel.rename(name, cx);
        });
        cx.emit(WorkspaceEvent::PersistRequested);
        true
    }

    pub fn next_surface(
        &mut self,
        surface_index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((tab, position)) = self.resolve_tab_target(surface_index, cx) else {
            return false;
        };

        tab.update(cx, |tab_panel, cx| {
            if tab_panel.active_index() != position {
                tab_panel.set_active_index(position, window, cx);
            }
            tab_panel.next_tab(window, cx);
        });
        true
    }

    pub fn prev_surface(
        &mut self,
        surface_index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((tab, position)) = self.resolve_tab_target(surface_index, cx) else {
            return false;
        };

        tab.update(cx, |tab_panel, cx| {
            if tab_panel.active_index() != position {
                tab_panel.set_active_index(position, window, cx);
            }
            tab_panel.prev_tab(window, cx);
        });
        true
    }

    pub fn toggle_zoom_surface(
        &mut self,
        surface_index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<bool> {
        let (tab, position) = self.resolve_tab_target(surface_index, cx)?;
        let is_zoomed = self.dock_area.read(cx).is_zoomed();

        tab.update(cx, |tab_panel, cx| {
            if tab_panel.active_index() != position {
                tab_panel.set_active_index(position, window, cx);
            }
        });

        if is_zoomed {
            self.dock_area.update(cx, |dock_area, cx| {
                dock_area.set_zoomed_out(window, cx);
            });
        } else {
            self.dock_area.update(cx, |dock_area, cx| {
                dock_area.set_zoomed_in(tab, window, cx);
            });
        }

        cx.notify();
        Some(!is_zoomed)
    }

    pub fn select_all_in_surface(
        &self,
        surface_index: Option<usize>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(panel) = self
            .resolve_surface_target(surface_index, cx)
            .map(|target| target.panel)
        else {
            return false;
        };

        panel.update(cx, |panel, cx| {
            panel.select_all(cx);
        });
        true
    }

    pub fn copy_selection_from_surface(
        &self,
        surface_index: Option<usize>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(panel) = self
            .resolve_surface_target(surface_index, cx)
            .map(|target| target.panel)
        else {
            return false;
        };

        panel.update(cx, |panel, cx| {
            panel.copy_selection(cx);
        });
        true
    }

    pub fn copy_all_from_surface(
        &self,
        surface_index: Option<usize>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(panel) = self
            .resolve_surface_target(surface_index, cx)
            .map(|target| target.panel)
        else {
            return false;
        };

        panel.update(cx, |panel, cx| {
            panel.copy_all(cx);
        });
        true
    }

    pub fn paste_into_surface(&self, surface_index: Option<usize>, cx: &mut Context<Self>) -> bool {
        let Some(panel) = self
            .resolve_surface_target(surface_index, cx)
            .map(|target| target.panel)
        else {
            return false;
        };

        panel.update(cx, |panel, cx| {
            panel.paste_from_system_clipboard(cx);
        });
        true
    }

    pub fn read_surface_text(
        &self,
        surface_index: Option<usize>,
        scope: SurfaceReadScope,
        cx: &App,
    ) -> Option<String> {
        let panel = self
            .resolve_surface_target(surface_index, cx)
            .map(|target| target.panel)?;

        Some(match scope {
            SurfaceReadScope::Buffer => panel.read(cx).buffer_text(cx),
            SurfaceReadScope::Visible => panel.read(cx).visible_text(cx),
        })
    }

    fn target_tab_panel(&self, window: Option<&Window>, cx: &App) -> Option<Entity<TabPanel>> {
        self.dock_area.read(cx).target_tab_panel(window, cx)
    }

    #[allow(dead_code)]
    fn target_terminal_panel(&self, cx: &App) -> Option<Entity<TerminalPanel>> {
        let tab_panel = self.target_tab_panel(None, cx)?;
        let active_panel = tab_panel.read(cx).active_panel(cx)?;
        if active_panel.panel_name(cx) != "TerminalPanel" {
            return None;
        }
        Some(Entity::<TerminalPanel>::from(active_panel.as_ref()))
    }

    fn resolve_tab_target(
        &self,
        index: Option<usize>,
        cx: &App,
    ) -> Option<(Entity<TabPanel>, usize)> {
        let target = self
            .target_tab_panel(None, cx)
            .and_then(|tab| tab.read(cx).active_panel(cx))
            .map(|panel| panel.view().entity_id());
        for tab in self.dock_area.read(cx).items().all_tab_panels(cx) {
            for (position, panel) in tab.read(cx).panels().iter().enumerate() {
                let panel_index = match panel.panel_name(cx) {
                    "TerminalPanel" => Some(
                        Entity::<TerminalPanel>::from(panel.as_ref())
                            .read(cx)
                            .index(),
                    ),
                    "BrowserPanel" => {
                        Some(Entity::<BrowserPanel>::from(panel.as_ref()).read(cx).index)
                    }
                    _ => None,
                };
                if index.map_or(Some(panel.view().entity_id()) == target, |index| {
                    panel_index == Some(index)
                }) {
                    return Some((tab.clone(), position));
                }
            }
        }
        None
    }

    fn resolve_surface_target(
        &self,
        surface_index: Option<usize>,
        cx: &App,
    ) -> Option<SurfaceTarget> {
        let targets = self.surface_targets(cx);
        match surface_index {
            Some(index) => targets
                .into_iter()
                .find(|target| target.panel.read(cx).index() == index),
            None => targets.into_iter().find(|target| target.target),
        }
    }

    fn surface_targets(&self, cx: &App) -> Vec<SurfaceTarget> {
        let target_panel_id = self
            .target_terminal_panel(cx)
            .map(|panel| panel.entity_id());
        let tab_panels = self.dock_area.read(cx).items().all_tab_panels(cx);
        let mut result = Vec::new();

        for tab_panel in tab_panels {
            let (active_index, panels) = {
                let tab_panel_ref = tab_panel.read(cx);
                (
                    tab_panel_ref.active_index(),
                    tab_panel_ref.panels().to_vec(),
                )
            };

            for (tab_position, panel_view) in panels.into_iter().enumerate() {
                if panel_view.panel_name(cx) != "TerminalPanel" {
                    continue;
                }

                let panel = Entity::<TerminalPanel>::from(panel_view.as_ref());
                result.push(SurfaceTarget {
                    tab_panel: tab_panel.clone(),
                    tab_position,
                    active_tab: tab_position == active_index,
                    target: Some(panel.entity_id()) == target_panel_id,
                    panel,
                });
            }
        }

        result
    }

    fn current_next_terminal_index(&self, cx: &App) -> usize {
        next_terminal_index_from_state(Some(&self.dock_area.read(cx).dump(cx)))
    }

    fn new_terminal_panel_view(
        &mut self,
        cwd: Option<&std::path::Path>,
        cx: &mut Context<Self>,
    ) -> (usize, Entity<TerminalPanel>, Arc<dyn PanelView>) {
        let index = self.current_next_terminal_index(cx);
        self.next_terminal_index = index + 1;

        let connection = self.ssh.as_ref().map(|connection| {
            connection.for_new_pane(cwd.map(|path| path.to_string_lossy().into_owned()))
        });
        let panel = Self::create_terminal_panel(cwd, connection.as_ref(), index, cx);
        let panel_view: Arc<dyn PanelView> = Arc::new(panel.clone());
        (index, panel, panel_view)
    }
}

impl EventEmitter<WorkspaceEvent> for Workspace {}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(theme::BG_PRIMARY))
            .child(self.dock_area.clone())
    }
}

fn next_terminal_index_from_state(state: Option<&DockAreaState>) -> usize {
    state
        .and_then(|state| max_terminal_index(&state.center))
        .map(|index| index + 1)
        .unwrap_or(1)
}

fn max_terminal_index(state: &PanelState) -> Option<usize> {
    let mut max_index = match &state.info {
        PanelInfo::Panel(value)
            if matches!(state.panel_name.as_str(), "TerminalPanel" | "BrowserPanel") =>
        {
            value
                .get("index")
                .and_then(|index| index.as_u64())
                .map(|index| index as usize)
        }
        _ => None,
    };

    for child in &state.children {
        if let Some(child_index) = max_terminal_index(child) {
            max_index = Some(match max_index {
                Some(current) => current.max(child_index),
                None => child_index,
            });
        }
    }

    max_index
}

fn split_dock_item(
    item: DockItem,
    target_tab_panel_id: EntityId,
    new_item: DockItem,
    placement: Placement,
    dock_area: &WeakEntity<DockArea>,
    window: &mut Window,
    cx: &mut App,
) -> Option<DockItem> {
    match item {
        DockItem::Tabs {
            size,
            items,
            active_ix,
            view,
        } => {
            if view.entity_id() != target_tab_panel_id {
                return None;
            }

            let current_item = DockItem::Tabs {
                size,
                items,
                active_ix,
                view,
            };
            let split = match placement {
                Placement::Left => {
                    DockItem::h_split(vec![new_item, current_item], dock_area, window, cx)
                }
                Placement::Right => {
                    DockItem::h_split(vec![current_item, new_item], dock_area, window, cx)
                }
                Placement::Top => {
                    DockItem::v_split(vec![new_item, current_item], dock_area, window, cx)
                }
                Placement::Bottom => {
                    DockItem::v_split(vec![current_item, new_item], dock_area, window, cx)
                }
            };
            Some(match size {
                Some(size) => split.size(size),
                None => split,
            })
        }
        DockItem::Split {
            axis,
            size,
            items,
            sizes,
            ..
        } => {
            let mut replaced = false;
            let mut new_items = Vec::with_capacity(items.len());

            for child in items {
                if !replaced {
                    if let Some(split_child) = split_dock_item(
                        child.clone(),
                        target_tab_panel_id,
                        new_item.clone(),
                        placement,
                        dock_area,
                        window,
                        cx,
                    ) {
                        new_items.push(split_child);
                        replaced = true;
                        continue;
                    }
                }

                new_items.push(child);
            }

            if !replaced {
                return None;
            }

            let split = DockItem::split_with_sizes(axis, new_items, sizes, dock_area, window, cx);
            Some(match size {
                Some(size) => split.size(size),
                None => split,
            })
        }
        DockItem::Panel { .. } | DockItem::Tiles { .. } => None,
    }
}
