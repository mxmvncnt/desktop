//! Virtualized list of the files a pull request would change.

use gpui::{App, Entity, IntoElement, RenderOnce, Window, prelude::*, px, uniform_list};

use crate::repo::RepoModel;
use crate::shared::file_change_row::FileChangeRow;

#[derive(IntoElement)]
pub struct PrFileList {
    repo: Entity<RepoModel>,
}

impl PrFileList {
    pub fn new(repo: Entity<RepoModel>) -> Self {
        Self { repo }
    }
}

impl RenderOnce for PrFileList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let pr = &self.repo.read(cx).pr;
        let n = pr.comparison.as_ref().map_or(0, |c| c.files.len());
        let scroll = pr.files_scroll.clone();
        let repo = self.repo;
        uniform_list("pr-files", n, move |range, _w, cx| {
            let pr = &repo.read(cx).pr;
            let files = pr.comparison.as_ref().map(|c| c.files.as_slice()).unwrap_or_default();
            range
                .filter_map(|ix| {
                    let change = files.get(ix)?.clone();
                    let repo = repo.clone();
                    let on_select = move |_: &mut Window, cx: &mut App| repo.update(cx, |r, cx| r.select_pr_file(ix, cx));
                    Some(FileChangeRow::new("pr-file", ix, change, pr.selected_file == Some(ix), on_select))
                })
                .collect()
        })
        .track_scroll(&scroll)
        .px(px(6.0))
        .flex_1()
    }
}
