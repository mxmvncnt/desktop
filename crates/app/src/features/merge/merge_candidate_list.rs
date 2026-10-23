//! Virtualized list of branches the merge dialog can merge from.

use gd_git::{Branch, BranchKind};
use gd_ui::{Icon, Theme, icon, pressable};
use gpui::{
    App, Entity, IntoElement, ParentElement, RenderOnce, StatefulInteractiveElement, Styled,
    UniformListScrollHandle, Window, div, px, uniform_list,
};

use crate::shared::relative_time::relative_time;
use crate::workspace::Workspace;

pub const ROW_HEIGHT: f32 = 30.0;

#[derive(IntoElement)]
pub struct MergeCandidateList {
    branches: Vec<Branch>,
    selected: usize,
    scroll: UniformListScrollHandle,
    ws: Entity<Workspace>,
}

impl MergeCandidateList {
    pub fn new(branches: Vec<Branch>, selected: usize, scroll: UniformListScrollHandle, ws: Entity<Workspace>) -> Self {
        Self { branches, selected, scroll, ws }
    }
}

impl RenderOnce for MergeCandidateList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let n = self.branches.len();
        if n == 0 {
            return empty(cx).into_any_element();
        }
        let (rows, ws, selected) = (self.branches, self.ws, self.selected);
        uniform_list("merge-branches", n, move |range, _w, cx| {
            range
                .filter_map(|i| Some(row(i, rows.get(i)?, i == selected, ws.clone(), cx)))
                .collect()
        })
        .track_scroll(&self.scroll)
        .size_full()
        .into_any_element()
    }
}

fn empty(cx: &App) -> impl IntoElement + use<> {
    let theme = Theme::of(cx);
    div()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .text_color(theme.text_faint)
        .child("No matching branches")
}

fn row(i: usize, b: &Branch, selected: bool, ws: Entity<Workspace>, cx: &App) -> impl IntoElement + use<> {
    let theme = Theme::of(cx).clone();
    let kind_icon = if b.kind == BranchKind::Remote { Icon::Cloud } else { Icon::GitBranch };
    pressable(("merge-branch", i), selected, cx)
        .flex()
        .items_center()
        .gap(px(10.0))
        .h(px(ROW_HEIGHT))
        .w_full()
        .px(px(8.0))
        .rounded(px(6.0))
        .child(icon(kind_icon).size(px(14.0)).text_color(theme.text_faint))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(if selected { theme.text } else { theme.text_muted })
                .child(b.name.clone()),
        )
        .child(
            div()
                .flex_none()
                .text_size(px(11.0))
                .text_color(theme.text_faint)
                .child(relative_time(b.tip_time)),
        )
        .on_click(move |_, _, cx| ws.update(cx, |this, cx| this.merge_select(i, cx)))
}
