use std::sync::Arc;

use gpui::*;
use gpui_component::Placement;
use gpui_component::dock::{
    DockArea, DockAreaState, DockEvent, DockItem, DockPlacement, PanelInfo, PanelState, PanelStyle,
    PanelView, TabPanel,
};
use uuid::Uuid;

use crate::session::WorkspaceSession;
use crate::terminal_surface::spawn_terminal_view;
use crate::terminal_panel::TerminalPanel;
use crate::theme;

pub enum WorkspaceEvent {
    PersistRequested,
}

pub struct Workspace {
    pub name: String,
    pub dock_area: Entity<DockArea>,
    pub unread_count: usize,
    pub color: Option<u32>,
    pub git_branch: Option<String>,
    pub cwd: Option<String>,
    next_terminal_index: usize,
    _dock_subscription: Subscription,
}

impl Workspace {
    pub fn new(
        name: String,
        cwd: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(name, cwd, None, Some(1), window, cx)
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

        Self::build(
            session.name,
            cwd,
            session.dock_area,
            Some(next_terminal_index),
            window,
            cx,
        )
    }

    pub fn to_session(&self, cx: &App) -> WorkspaceSession {
        WorkspaceSession {
            name: self.name.clone(),
            cwd: self.cwd.clone().unwrap_or_default(),
            dock_area: Some(self.dock_area.read(cx).dump(cx)),
            next_terminal_index: Some(self.current_next_terminal_index(cx)),
        }
    }

    fn build(
        name: String,
        cwd: Option<String>,
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
            let center = Self::default_center(initial_cwd, &dock_area, window, cx);
            dock_area.update(cx, |area, cx| {
                area.set_center(center, window, cx);
            });
        }

        Self {
            name,
            dock_area,
            unread_count: 0,
            color: None,
            git_branch: None,
            cwd,
            next_terminal_index: next_terminal_index.unwrap_or(1).max(1),
            _dock_subscription: dock_subscription,
        }
    }

    fn default_center(
        cwd: Option<&std::path::Path>,
        dock_area: &Entity<DockArea>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> DockItem {
        let panel = Self::create_terminal_panel(cwd, 0, cx);
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
        index: usize,
        cx: &mut Context<Self>,
    ) -> Entity<TerminalPanel> {
        cx.new(|cx| {
            TerminalPanel::from_cwd(cwd, index, cx).unwrap_or_else(|_| {
                let term =
                    spawn_terminal_view(&mut **cx, None, None).expect("failed to spawn terminal");
                TerminalPanel::new(term, index, None, cx)
            })
        })
    }

    pub fn add_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cwd_path = self.cwd.as_deref().map(std::path::Path::new);
        let index = self.current_next_terminal_index(cx);
        self.next_terminal_index = index + 1;

        let panel = Self::create_terminal_panel(cwd_path, index, cx);
        let panel_view: Arc<dyn PanelView> = Arc::new(panel);

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
        let cwd_path = self.cwd.as_deref().map(std::path::Path::new);
        let index = self.current_next_terminal_index(cx);
        self.next_terminal_index = index + 1;

        let panel = Self::create_terminal_panel(cwd_path, index, cx);
        let panel_view: Arc<dyn PanelView> = Arc::new(panel);

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

    pub fn write_to_target_terminal(&self, text: &str, cx: &mut App) {
        if let Some(panel) = self.target_terminal_panel(cx) {
            panel.update(cx, |panel, cx| {
                panel.write_to_terminal(text.as_bytes(), &mut **cx);
            });
        }
    }

    fn target_tab_panel(&self, window: Option<&Window>, cx: &App) -> Option<Entity<TabPanel>> {
        self.dock_area.read(cx).target_tab_panel(window, cx)
    }

    fn target_terminal_panel(&self, cx: &App) -> Option<Entity<TerminalPanel>> {
        let tab_panel = self.target_tab_panel(None, cx)?;
        let active_panel = tab_panel.read(cx).active_panel(cx)?;
        Some(Entity::<TerminalPanel>::from(active_panel.as_ref()))
    }

    fn current_next_terminal_index(&self, cx: &App) -> usize {
        next_terminal_index_from_state(Some(&self.dock_area.read(cx).dump(cx)))
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
        PanelInfo::Panel(value) if state.panel_name == "TerminalPanel" => value
            .get("index")
            .and_then(|index| index.as_u64())
            .map(|index| index as usize),
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
