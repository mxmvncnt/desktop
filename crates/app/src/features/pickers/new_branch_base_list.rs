//! Scrollable list of branches to start a new branch from.

use gd_git::BranchKind;
use gd_ui::{Icon, Theme, icon, pressable};
use gpui::{
    App, Entity, IntoElement, ParentElement, RenderOnce, StatefulInteractiveElement, Styled,
    Window, div, prelude::FluentBuilder, px, uniform_list,
};

use super::new_branch_base::NewBranchBase;
use crate::workspace::Workspace;

const ROW_H: f32 = 28.0;
const VISIBLE_ROWS: f32 = 6.0;

#[derive(IntoElement)]
pub struct NewBranchBaseList {
    branches: Vec<gd_git::Branch>,
    current: String,
    selected: String,
    ws: Entity<Workspace>,
}

impl NewBranchBaseList {
    pub fn new(base: &NewBranchBase) -> Self {
        Self {
            branches: base.branches.clone(),
            current: base.current.clone(),
            selected: base.effective().to_string(),
            ws: base.ws.clone(),
        }
    }
}

impl RenderOnce for NewBranchBaseList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let n = self.branches.len();
        let height = (n as f32).min(VISIBLE_ROWS) * ROW_H + 8.0;
        let Self { branches, current, selected, ws } = self;
        div()
            .h(px(height))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border)
            .child(
                uniform_list("new-branch-base-list", n, move |range, _w, cx| {
                    range
                        .filter_map(|i| Some((i, branches.get(i)?.clone())))
                        .map(|(i, b)| base_row(i, b, &current, &selected, &ws, cx))
                        .collect()
                })
                .p(px(4.0))
                .size_full(),
            )
    }
}

/// Hot row (virtualized): a plain fn, not a component.
fn base_row(
    i: usize,
    b: gd_git::Branch,
    current: &str,
    selected: &str,
    ws: &Entity<Workspace>,
    cx: &mut App,
) -> gpui::AnyElement {
    let theme = Theme::of(cx).clone();
    let ws = ws.clone();
    let is_selected = b.name == selected;
    let pick = (b.name != current).then(|| b.name.clone());
    let glyph = if b.kind == BranchKind::Remote { Icon::Cloud } else { Icon::GitBranch };
    pressable(("new-branch-base-row", i), false, cx)
        .flex()
        .items_center()
        .gap(px(8.0))
        .h(px(ROW_H))
        .w_full()
        .px(px(6.0))
        .rounded(px(5.0))
        .text_size(px(13.0))
        .child(icon(glyph).size(px(14.0)).text_color(theme.text_faint))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(if is_selected { theme.text } else { theme.text_muted })
                .child(b.name.clone()),
        )
        .when(is_selected, |d| d.child(icon(Icon::Check).size(px(14.0)).text_color(theme.text)))
        .on_click(move |_, _, cx| {
            let pick = pick.clone();
            ws.update(cx, |this, cx| this.set_new_branch_base(pick, cx))
        })
        .into_any_element()
}
