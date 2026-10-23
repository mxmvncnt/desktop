//! "Based on" selector of the new-branch popover: a button showing the start
//! point (current branch by default) that expands into the branch list.

use gd_git::Branch;
use gd_ui::{Icon, Theme, icon, pressable};
use gpui::{
    App, Entity, IntoElement, ParentElement, RenderOnce, StatefulInteractiveElement, Styled,
    Window, div, px,
};

use super::new_branch_base_list::NewBranchBaseList;
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct NewBranchBase {
    /// Checked-out branch (the default start point).
    pub current: String,
    /// Explicit start point, `None` = current branch.
    pub chosen: Option<String>,
    /// Whether the branch list is expanded.
    pub open: bool,
    pub branches: Vec<Branch>,
    pub ws: Entity<Workspace>,
}

impl NewBranchBase {
    /// Name of the branch the new one will start from.
    pub fn effective(&self) -> &str {
        self.chosen.as_deref().unwrap_or(&self.current)
    }
}

impl RenderOnce for NewBranchBase {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let ws = self.ws.clone();
        let chevron = if self.open { Icon::ChevronUp } else { Icon::ChevronDown };
        let label = if self.chosen.is_none() {
            format!("{} (current)", self.current)
        } else {
            self.effective().to_string()
        };
        let trigger = pressable("new-branch-base", self.open, cx)
            .flex()
            .items_center()
            .gap(px(8.0))
            .h(px(30.0))
            .px(px(8.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border)
            .child(icon(Icon::GitBranch).size(px(14.0)).text_color(theme.text_faint))
            .child(div().flex_1().min_w_0().truncate().text_color(theme.text).child(label))
            .child(icon(chevron).size(px(14.0)).text_color(theme.text_faint))
            .on_click(move |_, _, cx| ws.update(cx, |this, cx| this.toggle_new_branch_base(cx)));
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(div().text_size(px(12.0)).text_color(theme.text_muted).child("Based on"))
            .child(trigger)
            .children(self.open.then(|| NewBranchBaseList::new(&self)))
    }
}
