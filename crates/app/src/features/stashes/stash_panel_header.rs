//! Header of the stash panel: collapse toggle and the branch filter button.

use gd_ui::{Button, Icon, Theme, icon, pressable};
use gpui::{App, Entity, IntoElement, RenderOnce, Window, div, prelude::*, px};

use crate::repo::RepoModel;

#[derive(IntoElement)]
pub struct StashPanelHeader {
    repo: Entity<RepoModel>,
    /// Stashes visible under the current filter.
    shown: usize,
    collapsed: bool,
    all_branches: bool,
}

impl StashPanelHeader {
    pub fn new(repo: Entity<RepoModel>, shown: usize, collapsed: bool, all_branches: bool) -> Self {
        Self { repo, shown, collapsed, all_branches }
    }
}

impl RenderOnce for StashPanelHeader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let (repo_a, repo_b) = (self.repo.clone(), self.repo);
        let chevron = if self.collapsed { Icon::ChevronRight } else { Icon::ChevronDown };
        div()
            .flex()
            .flex_none()
            .items_center()
            .h(px(34.0))
            .pl(px(6.0))
            .pr(px(8.0))
            .gap(px(4.0))
            .child(
                pressable("stash-toggle", false, cx)
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap(px(6.0))
                    .h(px(26.0))
                    .px(px(6.0))
                    .rounded(px(6.0))
                    .child(icon(chevron).size(px(12.0)).text_color(theme.text_faint))
                    .child(icon(Icon::Archive).size(px(14.0)).text_color(theme.text_muted))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.text_muted)
                            .child(format!("Stashes ({})", self.shown)),
                    )
                    .on_click(move |_, _, cx| repo_a.update(cx, |r, cx| r.toggle_stash_panel(cx))),
            )
            .child(
                Button::new("stash-filter")
                    .ghost()
                    .compact()
                    .label(if self.all_branches { "All branches" } else { "This branch" })
                    .on_click(move |_, _, cx| repo_b.update(cx, |r, cx| r.toggle_stash_filter(cx))),
            )
    }
}
