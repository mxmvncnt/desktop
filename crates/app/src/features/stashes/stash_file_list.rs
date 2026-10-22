//! Virtualized list of the files changed by the selected stash.

use gpui::{App, Entity, IntoElement, RenderOnce, Window, prelude::*, px, uniform_list};

use super::stash_file_row::StashFileRow;
use crate::repo::RepoModel;

#[derive(IntoElement)]
pub struct StashFileList {
    repo: Entity<RepoModel>,
}

impl StashFileList {
    pub fn new(repo: Entity<RepoModel>) -> Self {
        Self { repo }
    }
}

impl RenderOnce for StashFileList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let st = &self.repo.read(cx).stashes;
        let (n, scroll) = (st.files.len(), st.files_scroll.clone());
        let repo = self.repo;
        uniform_list("stash-files", n, move |range, _w, cx| {
            let st = &repo.read(cx).stashes;
            range
                .filter_map(|ix| {
                    let change = st.files.get(ix)?.clone();
                    Some(StashFileRow::new(ix, change, st.selected_file == Some(ix), repo.clone()))
                })
                .collect()
        })
        .track_scroll(&scroll)
        .px(px(6.0))
        .flex_1()
    }
}
