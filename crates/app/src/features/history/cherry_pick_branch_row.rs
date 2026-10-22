//! One branch choice in the cherry-pick dialog.

use gd_ui::{Icon, Theme, icon, pressable};
use gpui::{App, IntoElement, RenderOnce, Window, div, prelude::*, px};

#[derive(IntoElement)]
pub struct CherryPickBranchRow {
    ix: usize,
    name: String,
    selected: bool,
    /// The checked-out branch.
    current: bool,
    on_select: Box<dyn Fn(&mut Window, &mut App)>,
}

impl CherryPickBranchRow {
    pub fn new(
        ix: usize,
        name: String,
        selected: bool,
        current: bool,
        on_select: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self { ix, name, selected, current, on_select: Box::new(on_select) }
    }
}

impl RenderOnce for CherryPickBranchRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let on_select = self.on_select;
        pressable(("cp-branch", self.ix), self.selected, cx)
            .flex()
            .items_center()
            .gap(px(8.0))
            .h(px(30.0))
            .px(px(8.0))
            .rounded(px(6.0))
            .child(icon(Icon::GitBranch).size(px(14.0)).text_color(theme.text_faint))
            .child(div().flex_1().min_w_0().truncate().text_size(px(13.0)).child(self.name))
            .when(self.current, |d| d.child(div().text_size(px(11.5)).text_color(theme.text_faint).child("current")))
            .when(self.selected, |d| d.child(icon(Icon::Check).size(px(14.0)).text_color(theme.text)))
            .on_click(move |_, w, cx| on_select(w, cx))
    }
}
