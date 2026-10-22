//! `impl Workspace`: Up/Down selection stepping in the file and commit lists.

use gpui::{Context, ScrollStrategy};

use super::state::Workspace;
use crate::repo::Tab;

impl Workspace {
    pub(super) fn select_step(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.popover.is_some() {
            self.step_popover_selection(delta, cx);
            return;
        }
        let Some(repo) = self.repo.clone() else { return };
        repo.update(cx, |r, cx| match r.tab {
            Tab::Changes => self.step_change(r, delta, cx),
            Tab::History => self.step_commit(r, delta, cx),
        });
    }

    fn step_change(&self, r: &mut crate::repo::RepoModel, delta: isize, cx: &mut Context<crate::repo::RepoModel>) {
        if let Some(ix) = step(r.selected_change, r.changes().len(), delta) {
            r.select_change(ix, cx);
            self.changes_scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
        }
    }

    fn step_commit(&self, r: &mut crate::repo::RepoModel, delta: isize, cx: &mut Context<crate::repo::RepoModel>) {
        if let Some(ix) = step(r.selected_commit, r.commits.len(), delta) {
            r.select_commit(ix, cx);
            self.history_scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
            if ix + 20 >= r.commits.len() {
                r.load_more_history(cx);
            }
        }
    }
}

/// Next selection index after moving `delta` rows, clamped to the list.
fn step(cur: Option<usize>, len: usize, delta: isize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(cur.map(|c| c as isize + delta).unwrap_or(0).clamp(0, len as isize - 1) as usize)
}
