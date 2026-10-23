//! Virtualized, infinitely-paged list of commits (History tab sidebar).

use gd_ui::Theme;
use gpui::{App, Entity, IntoElement, RenderOnce, UniformListScrollHandle, Window, div, prelude::*, px, uniform_list};

use super::commit_row::CommitRow;
use crate::shared::overscroll::{Overscroll, ScrollProbe};
use crate::repo::RepoModel;
use crate::workspace::{NavList, Workspace, nav_focus};

/// Load the next page when the visible range gets this close to the end.
const PAGE_AHEAD: usize = 30;

#[derive(IntoElement)]
pub struct CommitList {
    repo: Entity<RepoModel>,
    ws: Entity<Workspace>,
    scroll: UniformListScrollHandle,
}

impl CommitList {
    pub fn new(repo: Entity<RepoModel>, ws: Entity<Workspace>, scroll: UniformListScrollHandle) -> Self {
        Self { repo, ws, scroll }
    }
}

impl RenderOnce for CommitList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let n = self.repo.read(cx).commits.len();
        if n == 0 {
            return empty(cx).into_any_element();
        }
        let (repo, ws) = (self.repo, self.ws);
        let nav_ws = ws.clone();
        let probe = ScrollProbe::from(&self.scroll);
        let list = uniform_list("history", n, move |range, _w, cx| {
            // Infinite scroll: page in more commits near the end.
            if range.end + PAGE_AHEAD >= n {
                repo.update(cx, |r, cx| r.load_more_history(cx));
            }
            range.filter_map(|ix| CommitRow::from_model(ix, &repo, &ws, cx)).collect()
        })
        .track_scroll(&self.scroll)
        .px(px(6.0))
        .flex_1();
        nav_focus(div(), NavList::Commits, nav_ws).flex().flex_col().flex_1().min_h_0().py(px(4.0)).child(Overscroll::new("history-overscroll", probe, list)).into_any_element()
    }
}

fn empty(cx: &App) -> impl IntoElement + use<> {
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .text_color(Theme::of(cx).text_faint)
        .child("No commits yet")
}
