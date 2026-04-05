use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::dock::{Panel, PanelControl, PanelEvent, PanelState, TabPanel};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::menu::{ContextMenuExt, PopupMenu, PopupMenuItem};
use gpui_component::{IconName, Sizable};
use gpui_terminal::TerminalView;

use crate::terminal_surface::spawn_terminal_view;

/// A terminal panel that implements the Panel trait for use in DockArea.
///
/// Focus is delegated to the inner TerminalView — TabPanel uses this
/// to focus the terminal when this panel becomes active.
pub struct TerminalPanel {
    terminal: Entity<TerminalView>,
    name: Option<String>,
    index: usize,
    cwd: Option<String>,
    tab_panel: Option<WeakEntity<TabPanel>>,
    rename_editor: Option<Entity<InputState>>,
    rename_subscription: Option<Subscription>,
    focus_subscription: Option<Subscription>,
}

impl TerminalPanel {
    pub fn new(
        terminal: Entity<TerminalView>,
        index: usize,
        cwd: Option<String>,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            terminal,
            name: None,
            index,
            cwd,
            tab_panel: None,
            rename_editor: None,
            rename_subscription: None,
            focus_subscription: None,
        }
    }

    pub fn from_cwd(
        cwd: Option<&std::path::Path>,
        index: usize,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<Self> {
        let terminal = spawn_terminal_view(&mut **cx, cwd, None)?;
        Ok(Self::new(
            terminal,
            index,
            cwd.map(|path| path.to_string_lossy().to_string()),
            cx,
        ))
    }

    pub fn set_name(&mut self, name: Option<String>) {
        self.name = name;
    }

    #[allow(dead_code)]
    pub fn write_to_terminal(&self, bytes: &[u8], cx: &mut App) {
        self.terminal.update(cx, |view, _cx| {
            view.write_to_pty(bytes);
        });
    }

    fn display_name(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| format!("Terminal {}", self.index + 1))
    }

    fn start_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.rename_editor.is_some() {
            return;
        }

        let current = self.display_name();
        let editor = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(&current, window, cx);
            state
        });

        let subscription = cx.subscribe(
            &editor,
            |panel: &mut Self, editor, event: &InputEvent, cx| match event {
                InputEvent::PressEnter { .. } | InputEvent::Blur => {
                    let text = editor.read(cx).text().to_string();
                    let text = text.trim().to_string();
                    panel.finish_rename((!text.is_empty()).then_some(text), cx);
                }
                _ => {}
            },
        );

        self.rename_editor = Some(editor);
        self.rename_subscription = Some(subscription);
        cx.notify();

        // Run after the editor is mounted so the caret and full-text selection
        // are both visible on first paint.
        let Some(editor) = self.rename_editor.clone() else {
            return;
        };
        cx.spawn_in(window, async move |_, cx| {
            let _ = cx.update(|window, cx| {
                editor.update(cx, |state, cx| {
                    state.focus_and_select_all(window, cx);
                });
            });
        })
        .detach();
    }

    fn finish_rename(&mut self, name: Option<String>, cx: &mut Context<Self>) {
        self.name = name;
        self.rename_editor = None;
        self.rename_subscription = None;
        cx.notify();
    }

    fn split_or_add(
        tab_panel: &Entity<TabPanel>,
        placement: Option<gpui_component::Placement>,
        cwd: Option<&str>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(dock_area) = tab_panel.read(cx).dock_area() else {
            return;
        };

        let index = next_terminal_index(&dock_area, cx);
        let cwd_path = cwd.map(std::path::Path::new);
        let panel = cx.new(|cx| {
            TerminalPanel::from_cwd(cwd_path, index, cx).unwrap_or_else(|_| {
                let term =
                    spawn_terminal_view(&mut **cx, None, None).expect("failed to spawn terminal");
                TerminalPanel::new(term, index, None, cx)
            })
        });
        let panel_view = std::sync::Arc::new(panel);

        tab_panel.update(cx, |tab_panel, cx| {
            if let Some(placement) = placement {
                tab_panel.add_panel_at(panel_view, placement, None, window, cx);
            } else {
                tab_panel.add_panel(panel_view, window, cx);
            }
        });
    }
}

impl Panel for TerminalPanel {
    fn panel_name(&self) -> &'static str {
        "TerminalPanel"
    }

    fn tab_name(&self, _cx: &App) -> Option<SharedString> {
        if self.rename_editor.is_some() {
            None
        } else {
            Some(self.display_name().into())
        }
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(editor) = &self.rename_editor {
            div()
                .min_w(px(140.0))
                .on_mouse_down(MouseButton::Left, |_event, _window, cx| {
                    cx.stop_propagation();
                })
                .child(
                    Input::new(editor)
                        .appearance(false)
                        .bordered(false)
                        .xsmall(),
                )
                .into_any_element()
        } else {
            SharedString::from(self.display_name()).into_any_element()
        }
    }

    fn closable(&self, _cx: &App) -> bool {
        true
    }

    fn zoomable(&self, _cx: &App) -> Option<PanelControl> {
        Some(PanelControl::Both)
    }

    fn title_suffix(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let terminal = self.terminal.clone();
        let cwd_for_new = self.cwd.clone();
        let cwd_for_split_right = self.cwd.clone();
        let cwd_for_split_down = self.cwd.clone();
        let tab_panel_for_new = self.tab_panel.clone();
        let tab_panel_for_split_right = self.tab_panel.clone();
        let tab_panel_for_split_down = self.tab_panel.clone();
        Some(
            div()
                .flex()
                .items_center()
                .gap(px(2.0))
                .child(
                    Button::new("tp-new-term")
                        .ghost()
                        .compact()
                        .icon(IconName::Plus)
                        .tooltip("New Terminal")
                        .on_click(move |_event, window, cx| {
                            if let Some(tab_panel) = tab_panel_for_new
                                .as_ref()
                                .and_then(|tab_panel| tab_panel.upgrade())
                            {
                                Self::split_or_add(
                                    &tab_panel,
                                    None,
                                    cwd_for_new.as_deref(),
                                    window,
                                    cx,
                                );
                            }
                        }),
                )
                .child(
                    Button::new("tp-split-right")
                        .ghost()
                        .compact()
                        .icon(IconName::PanelRight)
                        .tooltip("Split Right")
                        .on_click(move |_event, window, cx| {
                            if let Some(tab_panel) = tab_panel_for_split_right
                                .as_ref()
                                .and_then(|tab_panel| tab_panel.upgrade())
                            {
                                Self::split_or_add(
                                    &tab_panel,
                                    Some(gpui_component::Placement::Right),
                                    cwd_for_split_right.as_deref(),
                                    window,
                                    cx,
                                );
                            }
                        }),
                )
                .child(
                    Button::new("tp-split-down")
                        .ghost()
                        .compact()
                        .icon(IconName::PanelBottom)
                        .tooltip("Split Down")
                        .on_click(move |_event, window, cx| {
                            if let Some(tab_panel) = tab_panel_for_split_down
                                .as_ref()
                                .and_then(|tab_panel| tab_panel.upgrade())
                            {
                                Self::split_or_add(
                                    &tab_panel,
                                    Some(gpui_component::Placement::Bottom),
                                    cwd_for_split_down.as_deref(),
                                    window,
                                    cx,
                                );
                            }
                        }),
                )
                .child(
                    Button::new("tp-agent")
                        .ghost()
                        .compact()
                        .icon(IconName::Bot)
                        .tooltip("Launch Claude")
                        .on_click(move |_event, _window, cx| {
                            terminal.update(cx, |view, _cx| {
                                view.write_to_pty(b"claude\n");
                            });
                        }),
                ),
        )
    }

    fn dump(&self, _cx: &App) -> PanelState {
        let mut state = PanelState::new(self);
        state.info = gpui_component::dock::PanelInfo::Panel(serde_json::json!({
            "name": self.name,
            "index": self.index,
            "cwd": self.cwd,
        }));
        state
    }

    fn on_added_to(
        &mut self,
        tab_panel: WeakEntity<TabPanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tab_panel = Some(tab_panel);
        let terminal_focus = self.terminal.read(cx).focus_handle().clone();
        self.focus_subscription =
            Some(cx.on_focus(&terminal_focus, window, |panel, _window, cx| {
                let Some(tab_panel) = panel
                    .tab_panel
                    .as_ref()
                    .and_then(|tab_panel| tab_panel.upgrade())
                else {
                    return;
                };
                let Some(dock_area) = tab_panel.read(cx).dock_area() else {
                    return;
                };

                let tab_panel = tab_panel.downgrade();
                dock_area.update(cx, |dock_area, _cx| {
                    dock_area.remember_tab_panel(tab_panel.clone());
                });
            }));
    }

    fn on_removed(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.tab_panel = None;
        self.focus_subscription = None;
    }

    fn on_tab_double_click(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.start_rename(window, cx);
    }

    fn set_active(&mut self, active: bool, _window: &mut Window, cx: &mut Context<Self>) {
        if !active {
            self.terminal.update(cx, |view, cx| {
                view.deselect(cx);
            });
        }
    }

    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        let panel = cx.entity().clone();
        menu.item(
            gpui_component::menu::PopupMenuItem::new("Rename Tab")
                .icon(IconName::ALargeSmall)
                .on_click(move |_event, window, cx| {
                    panel.update(cx, |panel, cx| {
                        panel.start_rename(window, cx);
                    });
                }),
        )
    }
}

impl EventEmitter<PanelEvent> for TerminalPanel {}

impl Focusable for TerminalPanel {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        // Delegate to terminal's focus handle.
        // This is the key architectural pattern: TabPanel calls this to
        // get the focus handle, so when a tab is activated the terminal
        // itself receives focus — no custom focus management needed.
        self.terminal.read(cx).focus_handle().clone()
    }
}

impl Render for TerminalPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let panel = _cx.entity().clone();
        let terminal_for_menu = self.terminal.clone();
        let terminal_for_copy = self.terminal.clone();
        let terminal_for_copy_all = self.terminal.clone();
        let terminal_for_select_all = self.terminal.clone();
        let terminal_for_paste = self.terminal.clone();
        let tab_panel_for_new = self.tab_panel.clone();
        let tab_panel_for_split_right = self.tab_panel.clone();
        let tab_panel_for_split_down = self.tab_panel.clone();
        let tab_panel_for_close = self.tab_panel.clone();
        let cwd_for_new = self.cwd.clone();
        let cwd_for_split_right = self.cwd.clone();
        let cwd_for_split_down = self.cwd.clone();
        let panel_for_rename = panel.clone();
        let panel_for_close = panel.clone();

        div()
            .size_full()
            .context_menu(move |menu, _window, cx| {
                let has_selection = terminal_for_menu.read(cx).has_selection();
                let has_paste = cx
                    .read_from_clipboard()
                    .and_then(|clipboard| clipboard.text())
                    .is_some_and(|text| !text.is_empty());

                menu.item(
                    PopupMenuItem::new("Copy")
                        .icon(IconName::Copy)
                        .disabled(!has_selection)
                        .on_click({
                            let terminal = terminal_for_copy.clone();
                            move |_event, window, cx| {
                                terminal.read(cx).focus_handle().clone().focus(window);
                                terminal.update(cx, |view, cx| {
                                    view.copy_selection(cx);
                                });
                            }
                        }),
                )
                .item(PopupMenuItem::new("Copy All").on_click({
                    let terminal = terminal_for_copy_all.clone();
                    move |_event, window, cx| {
                        terminal.read(cx).focus_handle().clone().focus(window);
                        terminal.update(cx, |view, cx| {
                            view.copy_all(cx);
                        });
                    }
                }))
                .item(PopupMenuItem::new("Paste").disabled(!has_paste).on_click({
                    let terminal = terminal_for_paste.clone();
                    move |_event, window, cx| {
                        terminal.read(cx).focus_handle().clone().focus(window);
                        terminal.update(cx, |view, cx| {
                            view.paste_from_system_clipboard(cx);
                        });
                    }
                }))
                .item(PopupMenuItem::new("Select All").on_click({
                    let terminal = terminal_for_select_all.clone();
                    move |_event, window, cx| {
                        terminal.read(cx).focus_handle().clone().focus(window);
                        terminal.update(cx, |view, cx| {
                            view.select_all(cx);
                        });
                    }
                }))
                .separator()
                .item(
                    PopupMenuItem::new("New Terminal")
                        .icon(IconName::Plus)
                        .on_click({
                            let tab_panel = tab_panel_for_new.clone();
                            let cwd = cwd_for_new.clone();
                            move |_event, window, cx| {
                                if let Some(tab_panel) =
                                    tab_panel.as_ref().and_then(|tab_panel| tab_panel.upgrade())
                                {
                                    Self::split_or_add(
                                        &tab_panel,
                                        None,
                                        cwd.as_deref(),
                                        window,
                                        cx,
                                    );
                                }
                            }
                        }),
                )
                .item(
                    PopupMenuItem::new("Split Right")
                        .icon(IconName::PanelRight)
                        .on_click({
                            let tab_panel = tab_panel_for_split_right.clone();
                            let cwd = cwd_for_split_right.clone();
                            move |_event, window, cx| {
                                if let Some(tab_panel) =
                                    tab_panel.as_ref().and_then(|tab_panel| tab_panel.upgrade())
                                {
                                    Self::split_or_add(
                                        &tab_panel,
                                        Some(gpui_component::Placement::Right),
                                        cwd.as_deref(),
                                        window,
                                        cx,
                                    );
                                }
                            }
                        }),
                )
                .item(
                    PopupMenuItem::new("Split Down")
                        .icon(IconName::PanelBottom)
                        .on_click({
                            let tab_panel = tab_panel_for_split_down.clone();
                            let cwd = cwd_for_split_down.clone();
                            move |_event, window, cx| {
                                if let Some(tab_panel) =
                                    tab_panel.as_ref().and_then(|tab_panel| tab_panel.upgrade())
                                {
                                    Self::split_or_add(
                                        &tab_panel,
                                        Some(gpui_component::Placement::Bottom),
                                        cwd.as_deref(),
                                        window,
                                        cx,
                                    );
                                }
                            }
                        }),
                )
                .separator()
                .item(
                    PopupMenuItem::new("Rename Tab")
                        .icon(IconName::ALargeSmall)
                        .on_click({
                            let panel = panel_for_rename.clone();
                            move |_event, window, cx| {
                                panel.update(cx, |panel, cx| {
                                    panel.start_rename(window, cx);
                                });
                            }
                        }),
                )
                .item(PopupMenuItem::new("Close Tab").on_click({
                    let panel = panel_for_close.clone();
                    let tab_panel = tab_panel_for_close.clone();
                    move |_event, window, cx| {
                        if let Some(tab_panel) =
                            tab_panel.as_ref().and_then(|tab_panel| tab_panel.upgrade())
                        {
                            let panel_view: std::sync::Arc<dyn gpui_component::dock::PanelView> =
                                std::sync::Arc::new(panel.clone());
                            tab_panel.update(cx, |tab_panel, cx| {
                                tab_panel.remove_panel(panel_view, window, cx);
                            });
                        }
                    }
                }))
            })
            .child(self.terminal.clone())
    }
}

/// Register TerminalPanel with PanelRegistry for serialization support.
pub fn register(cx: &mut App) {
    use gpui_component::dock::{PanelInfo, PanelState, register_panel};

    register_panel(
        cx,
        "TerminalPanel",
        |_dock_area,
         panel_state: &PanelState,
         _panel_info: &PanelInfo,
         _window: &mut Window,
         cx: &mut App| {
            let index = if let PanelInfo::Panel(ref val) = panel_state.info {
                val.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize
            } else {
                0
            };
            let name = if let PanelInfo::Panel(ref val) = panel_state.info {
                val.get("name")
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            };
            let cwd = if let PanelInfo::Panel(ref val) = panel_state.info {
                val.get("cwd")
                    .and_then(|cwd| cwd.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            };

            let panel: Entity<TerminalPanel> = cx.new(|cx| {
                let cwd_path = cwd.as_deref().map(std::path::Path::new);
                let term = spawn_terminal_view(cx, cwd_path, None)
                    .or_else(|_| spawn_terminal_view(cx, None, None))
                    .expect("failed to spawn terminal");
                let mut p = TerminalPanel::new(term, index, cwd.clone(), cx);
                p.set_name(name);
                p
            });
            Box::new(panel)
        },
    );
}

fn next_terminal_index(dock_area: &Entity<gpui_component::dock::DockArea>, cx: &App) -> usize {
    let state = dock_area.read(cx).dump(cx);
    max_terminal_index(&state.center)
        .map(|index| index + 1)
        .unwrap_or(1)
}

fn max_terminal_index(state: &PanelState) -> Option<usize> {
    let mut max_index = match &state.info {
        gpui_component::dock::PanelInfo::Panel(value) if state.panel_name == "TerminalPanel" => {
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
