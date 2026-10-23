//! The collapsible "Stashes (N)" panel in the Changes sidebar.

use gd_ui::{Button, Theme};
use gpui::{AnyElement, App, Entity, IntoElement, RenderOnce, Window, div, prelude::*, px};

use super::stash_list::StashList;
use super::stash_panel_header::StashPanelHeader;
use crate::repo::RepoModel;
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct StashPanel {
    repo: Entity<RepoModel>,
    ws: Entity<Workspace>,
}

impl StashPanel {
    /// `None` when the repo has no stashes (the panel is hidden).
    pub fn new(repo: &Entity<RepoModel>, ws: Entity<Workspace>, cx: &App) -> Option<Self> {
        (!repo.read(cx).stashes.list.is_empty()).then(|| Self { repo: repo.clone(), ws })
    }
}

impl RenderOnce for StashPanel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let r = self.repo.read(cx);
        let st = &r.stashes;
        let branch = r.status.as_ref().and_then(|s| s.branch.clone());
        let visible = st.visible(branch.as_deref());
        let (shown, total, collapsed, all) = (visible.len(), st.list.len(), st.collapsed, st.all_branches);
        let body: Option<AnyElement> = if collapsed {
            None
        } else if shown == 0 {
            Some(empty_hint(&self.repo, total, &theme).into_any_element())
        } else {
            Some(StashList::new(self.repo.clone(), self.ws, visible, branch, all).into_any_element())
        };
        div()
            .flex()
            .flex_col()
            .flex_none()
            .border_t_1()
            .border_color(theme.border)
            .child(StashPanelHeader::new(self.repo, shown, collapsed, all))
            .children(body)
    }
}

/// Shown when the branch filter hides every stash.
fn empty_hint(repo: &Entity<RepoModel>, total: usize, theme: &Theme) -> impl IntoElement + use<> {
    let repo = repo.clone();
    div()
        .flex()
        .items_center()
        .justify_between()
        .px(px(14.0))
        .pb(px(10.0))
        .text_size(px(12.0))
        .text_color(theme.text_faint)
        .child("No stashes on this branch")
        .child(
            Button::new("stash-show-all")
                .ghost()
                .compact()
                .label(format!("Show all ({total})"))
                .on_click(move |_, _, cx| repo.update(cx, |r, cx| r.toggle_stash_filter(cx))),
        )
}
