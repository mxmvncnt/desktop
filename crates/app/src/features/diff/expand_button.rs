//! One expand arrow in a hunk header gutter.

use gd_git::Expansion;
use gd_ui::{Icon, Theme, icon, theme::DIFF_ROW_HEIGHT};
use gpui::{
    App, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};

use super::expander::ExpandTarget;
use crate::repo::RepoModel;

#[derive(IntoElement)]
pub struct ExpandButton {
    id: usize,
    icon: Icon,
    tip: &'static str,
    expansion: Expansion,
    repo: Entity<RepoModel>,
    target: ExpandTarget,
}

impl ExpandButton {
    pub fn new(
        id: usize,
        icon: Icon,
        tip: &'static str,
        expansion: Expansion,
        repo: Entity<RepoModel>,
        target: ExpandTarget,
    ) -> Self {
        Self { id, icon, tip, expansion, repo, target }
    }
}

impl RenderOnce for ExpandButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx);
        let (repo, target, e) = (self.repo, self.target, self.expansion);
        let group = SharedString::from(format!("expand-{}", self.id));
        let hover_fg = theme.on_accent;
        let hover_bg = theme.accent_strong;
        div()
            .id(("expand", self.id))
            .group(group.clone())
            .flex()
            .flex_1()
            .h(px(DIFF_ROW_HEIGHT))
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover_style(move |s| s.bg(hover_bg))
            .tooltip(gd_ui::tooltip(self.tip))
            .child(
                icon(self.icon)
                    .size(px(12.0))
                    .text_color(theme.text_faint)
                    .group_hover_style(group, move |s| s.text_color(hover_fg)),
            )
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                repo.update(cx, |r, cx| r.expand_diff(target, e, cx));
            })
    }
}
