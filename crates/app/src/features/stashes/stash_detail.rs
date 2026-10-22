//! Main pane while a stash is selected: header, file list and read-only diff.

use gd_ui::vdivider;
use gpui::{AnyElement, Context, Entity, IntoElement, SharedString, div, prelude::*, px};

use super::stash_file_list::StashFileList;
use super::stash_header::StashHeader;
use crate::actions::LIST;
use crate::repo::RepoModel;
use crate::features::diff::DiffPane;
use crate::workspace::Workspace;

impl Workspace {
    /// The diff renderer needs `&mut Workspace` (scroll state), so the detail
    /// pane is a method that composes the stateless pieces around it.
    pub fn render_stash_detail(&mut self, repo: &Entity<RepoModel>, cx: &mut Context<Self>) -> AnyElement {
        let r = repo.read(cx);
        let Some(stash) = r.stashes.selected_stash().cloned() else {
            return div().flex_1().into_any_element();
        };
        let st = &r.stashes;
        let here = stash.branch.is_some() && stash.branch == r.status.as_ref().and_then(|s| s.branch.clone());
        let file_key = st.selected_file.and_then(|ix| st.files.get(ix)).map(|f| f.path.clone()).unwrap_or_default();
        let (diff, hl, n_files) = (st.diff.clone(), st.hl.clone(), st.files.len());
        let diff_key = SharedString::from(format!("stash-diff-{}-{file_key}", stash.sha));
        let header = StashHeader::new(stash, n_files, here, repo.clone(), cx.entity());
        let file_list = div()
            .key_context(LIST)
            .flex()
            .flex_col()
            .flex_none()
            .w(px(260.0))
            .py(px(4.0))
            .child(StashFileList::new(repo.clone()));
        let diff = self.render_diff(diff, hl, diff_key, DiffPane::ReadOnly, cx);
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .child(header)
            .child(div().flex().flex_1().min_h_0().child(file_list).child(vdivider(cx)).child(diff))
            .into_any_element()
    }
}
