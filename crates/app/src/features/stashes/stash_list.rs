//! The virtualized list of stash rows inside the sidebar panel.

use gpui::{App, Entity, IntoElement, RenderOnce, Window, div, prelude::*, px, uniform_list};

use super::stash_row::{STASH_ROW_HEIGHT, StashRow};
use crate::shared::overscroll::{Overscroll, ScrollProbe};
use crate::repo::RepoModel;
use crate::workspace::{NavList, Workspace, nav_focus};

/// Rows visible before the panel's list scrolls.
const MAX_ROWS: usize = 4;

#[derive(IntoElement)]
pub struct StashList {
    repo: Entity<RepoModel>,
    ws: Entity<Workspace>,
    /// Indices into `StashState::list` that pass the branch filter.
    visible: Vec<usize>,
    current_branch: Option<String>,
    all_branches: bool,
}

impl StashList {
    pub fn new(
        repo: Entity<RepoModel>,
        ws: Entity<Workspace>,
        visible: Vec<usize>,
        current_branch: Option<String>,
        all_branches: bool,
    ) -> Self {
        Self { repo, ws, visible, current_branch, all_branches }
    }
}

impl RenderOnce for StashList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let n = self.visible.len();
        let st = &self.repo.read(cx).stashes;
        let (scroll, viewing, selected) = (st.list_scroll.clone(), st.viewing, st.selected.clone());
        let nav_ws = self.ws.clone();
        let Self { repo, ws, visible, current_branch, all_branches } = self;
        let probe = ScrollProbe::from(&scroll);
        let list = uniform_list("stashes", n, move |range, _w, cx| {
            let r = repo.read(cx);
            range
                .filter_map(|i| {
                    let s = r.stashes.list.get(*visible.get(i)?)?.clone();
                    let here = s.branch.is_some() && s.branch == current_branch;
                    let selected_here = viewing && selected.as_deref() == Some(s.sha.as_str());
                    Some(StashRow::new(s, selected_here, here, all_branches, repo.clone(), ws.clone()))
                })
                .collect()
        })
        .track_scroll(&scroll)
        .px(px(6.0))
        .size_full();
        nav_focus(div(), NavList::Stashes, nav_ws)
            .h(px(n.min(MAX_ROWS) as f32 * STASH_ROW_HEIGHT + 4.0))
            .flex_none()
            .child(Overscroll::new("stashes-overscroll", probe, list))
    }
}
