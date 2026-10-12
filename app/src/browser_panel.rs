use crate::browser::{BrowserEvent, BrowserView};
use gpui::*;
use gpui_component::dock::{
    Panel, PanelControl, PanelEvent, PanelInfo, PanelState, register_panel,
};
use uuid::Uuid;

pub struct BrowserPanel {
    pub id: Uuid,
    pub index: usize,
    pub browser: Entity<BrowserView>,
    pub name: Option<String>,
    active: bool,
    allowed: bool,
    tab: Option<WeakEntity<gpui_component::dock::TabPanel>>,
    _focus_subscription: Option<Subscription>,
    _subscription: Subscription,
}

impl BrowserPanel {
    pub fn new(
        index: usize,
        id: Uuid,
        url: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let browser = cx.new(|cx| BrowserView::new(url, window, cx));
        let subscription = cx.subscribe(&browser, |_, _, _: &BrowserEvent, cx| {
            cx.emit(PanelEvent::LayoutChanged);
            cx.notify();
        });
        Self {
            id,
            index,
            browser,
            name: None,
            active: true,
            allowed: true,
            tab: None,
            _focus_subscription: None,
            _subscription: subscription,
        }
    }

    pub fn title_text(&self, cx: &App) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| self.browser.read(cx).title.clone())
    }

    pub fn set_allowed(&mut self, allowed: bool, cx: &mut Context<Self>) {
        if self.allowed != allowed {
            self.allowed = allowed;
            self.browser.update(cx, |browser, _| {
                browser.set_visibility(self.active, allowed)
            });
        }
    }
}

impl EventEmitter<PanelEvent> for BrowserPanel {}
impl Focusable for BrowserPanel {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.browser.read(cx).focus_handle(cx)
    }
}
impl Panel for BrowserPanel {
    fn panel_name(&self) -> &'static str {
        "BrowserPanel"
    }
    fn tab_name(&self, cx: &App) -> Option<SharedString> {
        Some(self.title_text(cx).into())
    }
    fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        SharedString::from(self.title_text(cx))
    }
    fn zoomable(&self, _: &App) -> Option<PanelControl> {
        Some(PanelControl::Both)
    }
    fn set_active(&mut self, active: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.active = active;
        self.browser.update(cx, |browser, _| {
            browser.set_visibility(active, self.allowed)
        });
    }
    fn on_added_to(
        &mut self,
        tab: WeakEntity<gpui_component::dock::TabPanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tab = Some(tab);
        let focus = self.browser.read(cx).focus_handle(cx);
        self._focus_subscription = Some(cx.on_focus(&focus, window, |panel, _, cx| {
            let Some(tab) = panel.tab.as_ref().and_then(WeakEntity::upgrade) else {
                return;
            };
            let Some(area) = tab.read(cx).dock_area() else {
                return;
            };
            area.update(cx, |area, _| area.remember_tab_panel(tab.downgrade()));
        }));
    }
    fn dump(&self, cx: &App) -> PanelState {
        let mut state = PanelState::new(self);
        state.info = PanelInfo::Panel(
            serde_json::json!({"id":self.id.to_string(),"index":self.index,"url":self.browser.read(cx).url,"name":self.name}),
        );
        state
    }
}
impl Render for BrowserPanel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.browser.clone())
    }
}

pub fn register(cx: &mut App) {
    register_panel(cx, "BrowserPanel", |_, state, _, window, cx| {
        let info = if let PanelInfo::Panel(info) = &state.info {
            info.clone()
        } else {
            serde_json::json!({})
        };
        let id = info
            .get("id")
            .and_then(|id| id.as_str())
            .and_then(|id| Uuid::parse_str(id).ok())
            .unwrap_or_else(Uuid::new_v4);
        let index = info
            .get("index")
            .and_then(|index| index.as_u64())
            .unwrap_or(0) as usize;
        let url = info
            .get("url")
            .and_then(|url| url.as_str())
            .and_then(|url| crate::browser::normalize_url(url).ok())
            .unwrap_or_else(|| "about:blank".to_string());
        let name = info
            .get("name")
            .and_then(|name| name.as_str())
            .map(str::to_string);
        let panel = cx.new(|cx| {
            let mut panel = BrowserPanel::new(index, id, url, window, cx);
            panel.name = name;
            panel
        });
        Box::new(panel)
    });
}
