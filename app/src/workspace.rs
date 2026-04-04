use gpui::*;

use crate::pane::Pane;
use crate::terminal_surface::spawn_terminal_view;
use crate::theme;

#[derive(Clone, Copy, PartialEq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

pub enum LayoutNode {
    Leaf(Entity<Pane>),
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
}

pub struct Workspace {
    pub name: String,
    pub layout: LayoutNode,
    pub focused_pane: Entity<Pane>,
    pub unread_count: usize,
    pub zoomed: bool,
    pub color: Option<u32>,
    pub git_branch: Option<String>,
    pub cwd: Option<String>,
}

impl Workspace {
    pub fn new(name: String, pane: Entity<Pane>) -> Self {
        let focused = pane.clone();
        Self {
            name,
            layout: LayoutNode::Leaf(pane),
            focused_pane: focused,
            unread_count: 0,
            zoomed: false,
            color: None,
            git_branch: None,
            cwd: None,
        }
    }

    pub fn split(&mut self, direction: SplitDirection, cx: &mut App) {
        let focused = self.focused_pane.clone();
        let cwd_path = self.cwd.as_deref().map(std::path::Path::new);
        if let Ok(term) = spawn_terminal_view(cx, cwd_path, None) {
            let new_pane = cx.new(|cx| Pane::new(term, cx));
            self.layout = replace_pane_with_split(
                std::mem::replace(&mut self.layout, LayoutNode::Leaf(focused.clone())),
                &focused,
                direction,
                new_pane.clone(),
            );
            self.focused_pane = new_pane;
        }
    }

    pub fn has_splits(&self) -> bool {
        matches!(self.layout, LayoutNode::Split { .. })
    }

    pub fn toggle_zoom(&mut self, cx: &mut App) {
        // Only allow zoom when there are splits
        if !self.zoomed && !self.has_splits() {
            return;
        }

        self.zoomed = !self.zoomed;
        self.focused_pane.update(cx, |pane, _cx| {
            pane.is_zoomed = self.zoomed;
        });
        if !self.zoomed {
            for pane in self.panes() {
                pane.update(cx, |pane, _cx| {
                    pane.is_zoomed = false;
                });
            }
        }
    }

    pub fn panes(&self) -> Vec<Entity<Pane>> {
        let mut result = Vec::new();
        collect_panes(&self.layout, &mut result);
        result
    }

    fn render_layout_node(&self, node: &LayoutNode, cx: &mut Context<Self>) -> Div {
        match node {
            LayoutNode::Leaf(pane) => {
                let is_focused = pane == &self.focused_pane;
                let pane_clone = pane.clone();
                let mut d = div()
                    .size_full()
                    .on_mouse_down(MouseButton::Left, {
                        cx.listener(move |ws, _event, _window, cx| {
                            ws.focused_pane = pane_clone.clone();
                            cx.notify();
                        })
                    });
                if is_focused {
                    d = d.border_t_2().border_color(rgb(theme::ACCENT));
                } else {
                    d = d.border_t_2().border_color(gpui::transparent_black());
                }
                d.child(pane.clone())
            }
            LayoutNode::Split {
                direction,
                first,
                second,
                ..
            } => {
                let is_horizontal = *direction == SplitDirection::Horizontal;
                let first_child = self.render_layout_node(first, cx);
                let second_child = self.render_layout_node(second, cx);

                let mut divider = div()
                    .bg(rgb(theme::DIVIDER))
                    .flex_shrink_0();
                if is_horizontal {
                    divider = divider
                        .w(px(4.0))
                        .h_full()
                        .cursor(CursorStyle::ResizeLeftRight);
                } else {
                    divider = divider
                        .h(px(4.0))
                        .w_full()
                        .cursor(CursorStyle::ResizeUpDown);
                }

                let mut container = div().size_full().flex();
                if is_horizontal {
                    container = container.flex_row();
                } else {
                    container = container.flex_col();
                }

                container
                    .child(first_child.flex_1().overflow_hidden())
                    .child(divider)
                    .child(second_child.flex_1().overflow_hidden())
            }
        }
    }
}

fn collect_panes(node: &LayoutNode, out: &mut Vec<Entity<Pane>>) {
    match node {
        LayoutNode::Leaf(pane) => out.push(pane.clone()),
        LayoutNode::Split { first, second, .. } => {
            collect_panes(first, out);
            collect_panes(second, out);
        }
    }
}

fn replace_pane_with_split(
    node: LayoutNode,
    target: &Entity<Pane>,
    direction: SplitDirection,
    new_pane: Entity<Pane>,
) -> LayoutNode {
    match node {
        LayoutNode::Leaf(ref pane) if pane == target => LayoutNode::Split {
            direction,
            ratio: 0.5,
            first: Box::new(node),
            second: Box::new(LayoutNode::Leaf(new_pane)),
        },
        LayoutNode::Split {
            direction: d,
            ratio,
            first,
            second,
        } => LayoutNode::Split {
            direction: d,
            ratio,
            first: Box::new(replace_pane_with_split(*first, target, direction, new_pane.clone())),
            second: Box::new(replace_pane_with_split(*second, target, direction, new_pane)),
        },
        other => other,
    }
}

pub fn remove_pane_from_layout(node: LayoutNode, target: &Entity<Pane>) -> Option<LayoutNode> {
    match node {
        LayoutNode::Leaf(ref pane) if pane == target => None,
        LayoutNode::Leaf(_) => Some(node),
        LayoutNode::Split {
            first, second, ..
        } => {
            let first_result = remove_pane_from_layout(*first, target);
            let second_result = remove_pane_from_layout(*second, target);
            match (first_result, second_result) {
                (None, None) => None,
                (Some(node), None) | (None, Some(node)) => Some(node),
                (Some(f), Some(s)) => Some(LayoutNode::Split {
                    direction: SplitDirection::Horizontal,
                    ratio: 0.5,
                    first: Box::new(f),
                    second: Box::new(s),
                }),
            }
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Update can_zoom on all panes based on whether splits exist
        let has_splits = self.has_splits();
        for pane in self.panes() {
            pane.update(cx, |pane, _cx| {
                pane.can_zoom = has_splits;
            });
        }

        let content = if self.zoomed {
            div().size_full().child(self.focused_pane.clone())
        } else {
            self.render_layout_node(&self.layout, cx)
        };
        div()
            .size_full()
            .bg(rgb(theme::BG_PRIMARY))
            .child(content)
    }
}
