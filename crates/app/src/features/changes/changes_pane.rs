//! Main pane of the Changes tab. An `impl Workspace` method because the diff
//! renderer owns scroll state (`&mut Workspace`); the pieces around it are
//! stateless components.

use gpui::{AnyElement, Context, Entity, IntoElement, ParentElement, SharedString, Styled, div};

use super::file_header::FileHeader;
use super::no_changes::NoChanges;
use crate::features::diff::DiffPane;
use crate::features::merge::ConflictHint;
use crate::repo::RepoModel;
use crate::workspace::Workspace;
use gd_git::FileStatus;

impl Workspace {
    pub fn render_changes_main(&mut self, repo: &Entity<RepoModel>, cx: &mut Context<Self>) -> AnyElement {
        let r = repo.read(cx);
        if r.stashes.viewing {
            return self.render_stash_detail(repo, cx);
        }
        let Some(change) = r.selected_change.and_then(|ix| r.changes().get(ix)).cloned() else {
            return NoChanges::new(r).into_any_element();
        };
        let diff = r.change_diff.clone();
        let hl = r.change_hl.clone();
        let hint = (change.status == FileStatus::Conflicted)
            .then(|| ConflictHint::new(&change, diff.as_ref(), repo.clone(), cx));
        let header = FileHeader::new(&change, diff.as_deref(), cx.entity());
        let diff_id = SharedString::from(format!("change-diff-{}", change.path));
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .child(header)
            .children(hint)
            .child(self.render_diff(diff, hl, diff_id, DiffPane::Changes(repo.clone()), cx))
            .into_any_element()
    }
}
