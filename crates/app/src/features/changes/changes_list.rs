//! Virtualized list of changed files (or the "No local changes" note).

use gd_ui::Theme;
use gpui::{
    App, Entity, IntoElement, ParentElement, RenderOnce, Styled, UniformListScrollHandle, Window, div, px,
    uniform_list,
};

use super::change_row::change_row;
use crate::repo::RepoModel;
use crate::shared::overscroll::{Overscroll, ScrollProbe};
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct ChangesList {
    count: usize,
    repo: Entity<RepoModel>,
    ws: Entity<Workspace>,
    scroll: UniformListScrollHandle,
}

impl ChangesList {
    pub fn new(count: usize, repo: Entity<RepoModel>, ws: Entity<Workspace>, scroll: UniformListScrollHandle) -> Self {
        Self { count, repo, ws, scroll }
    }
}

impl RenderOnce for ChangesList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.count == 0 {
            let theme = Theme::of(cx);
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.text_faint)
                .text_size(px(12.0))
                .child("No local changes")
                .into_any_element();
        }
        let (repo, ws) = (self.repo, self.ws);
        let probe = ScrollProbe::from(&self.scroll);
        let list = uniform_list("changes", self.count, move |range, _window, cx| {
            let r = repo.read(cx);
            let rows: Vec<_> = range.filter_map(|ix| Some((ix, r.changes().get(ix)?.clone()))).collect();
            rows.into_iter().map(|(ix, change)| change_row(ix, change, &repo, &ws, cx)).collect()
        })
        .track_scroll(&self.scroll)
        .px(px(6.0))
        .flex_1();
        Overscroll::new("changes-overscroll", probe, list).into_any_element()
    }
}
