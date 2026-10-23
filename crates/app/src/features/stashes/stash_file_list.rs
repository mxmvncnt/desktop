//! Virtualized list of the files changed by the selected stash.

use gpui::{App, Entity, IntoElement, RenderOnce, Window, prelude::*, px, uniform_list};

use crate::shared::overscroll::{Overscroll, ScrollProbe};
use crate::repo::RepoModel;
use crate::shared::file_change_row::FileChangeRow;

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
        let probe = ScrollProbe::from(&scroll);
        let list = uniform_list("stash-files", n, move |range, _w, cx| {
            let st = &repo.read(cx).stashes;
            range
                .filter_map(|ix| {
                    let change = st.files.get(ix)?.clone();
                    let repo = repo.clone();
                    let on_select = move |_: &mut Window, cx: &mut App| repo.update(cx, |r, cx| r.select_stash_file(ix, cx));
                    Some(FileChangeRow::new("stash-file", ix, change, st.selected_file == Some(ix), on_select))
                })
                .collect()
        })
        .track_scroll(&scroll)
        .px(px(6.0))
        .flex_1();
        Overscroll::new("stash-files-overscroll", probe, list)
    }
}
