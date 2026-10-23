//! Virtualized list of the files changed by the selected commit.

use gpui::{App, Entity, IntoElement, RenderOnce, UniformListScrollHandle, Window, prelude::*, px, uniform_list};

use crate::shared::overscroll::{Overscroll, ScrollProbe};
use crate::repo::RepoModel;
use crate::shared::file_change_row::FileChangeRow;

#[derive(IntoElement)]
pub struct CommitFileList {
    repo: Entity<RepoModel>,
    scroll: UniformListScrollHandle,
}

impl CommitFileList {
    pub fn new(repo: Entity<RepoModel>, scroll: UniformListScrollHandle) -> Self {
        Self { repo, scroll }
    }
}

impl RenderOnce for CommitFileList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let n = self.repo.read(cx).commit_files.len();
        let repo = self.repo;
        let probe = ScrollProbe::from(&self.scroll);
        let list = uniform_list("commit-files", n, move |range, _w, cx| {
            let r = repo.read(cx);
            range
                .filter_map(|ix| {
                    let change = r.commit_files.get(ix)?.clone();
                    let repo = repo.clone();
                    let on_select = move |_: &mut Window, cx: &mut App| repo.update(cx, |r, cx| r.select_commit_file(ix, cx));
                    Some(FileChangeRow::new("commit-file", ix, change, r.selected_commit_file == Some(ix), on_select))
                })
                .collect()
        })
        .track_scroll(&self.scroll)
        .px(px(6.0))
        .flex_1();
        Overscroll::new("commit-files-overscroll", probe, list)
    }
}
