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
}

impl Workspace {
    pub fn new(name: String, pane: Entity<Pane>) -> Self {
        let focused = pane.clone();
        Self {
            name,
            layout: LayoutNode::Leaf(pane),
            focused_pane: focused,
            unread_count: 0,
        }
    }

    pub fn split(&mut self, direction: SplitDirection, cx: &mut App) {
        let focused = self.focused_pane.clone();
        if let Ok(term) = spawn_terminal_view(cx, None, None) {
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

    pub fn panes(&self) -> Vec<Entity<Pane>> {
        let mut result = Vec::new();
        collect_panes(&self.layout, &mut result);
        result
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
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(theme::BG_PRIMARY))
            .child(render_layout(&self.layout, &self.focused_pane))
    }
}

fn render_layout(node: &LayoutNode, focused: &Entity<Pane>) -> Div {
    match node {
        LayoutNode::Leaf(pane) => {
            let mut d = div().size_full();
            if pane == focused {
                d = d.border_1().border_color(rgb(theme::ACCENT));
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
            let first_child = render_layout(first, focused);
            let second_child = render_layout(second, focused);

            let mut divider = div().bg(rgb(theme::DIVIDER)).flex_shrink_0();
            if is_horizontal {
                divider = divider.w(px(4.0)).h_full();
            } else {
                divider = divider.h(px(4.0)).w_full();
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
