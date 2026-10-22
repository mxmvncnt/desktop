//! Multi-selection of history rows (Ctrl/Cmd toggle, Shift range).

use gpui::Context;

use super::state::ClickKind;
use crate::repo::RepoModel;

impl RepoModel {
    /// Indices of the multi-selected commits (ascending = newest first).
    pub fn multi_indices(&self) -> Vec<usize> {
        if self.history.multi.len() < 2 {
            return Vec::new();
        }
        self.commits
            .iter()
            .enumerate()
            .filter(|(_, c)| self.history.multi.contains(&c.sha))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn commit_selected(&self, ix: usize) -> bool {
        let multi = self.history.multi.len() >= 2;
        match self.commits.get(ix) {
            Some(c) if multi => self.history.multi.contains(&c.sha),
            Some(_) => self.selected_commit == Some(ix),
            None => false,
        }
    }

    /// Click on a history row.
    pub fn click_commit(&mut self, ix: usize, kind: ClickKind, cx: &mut Context<Self>) {
        let Some(sha) = self.commits.get(ix).map(|c| c.sha.clone()) else { return };
        match kind {
            ClickKind::Range => self.select_range(ix, cx),
            ClickKind::Toggle => self.toggle_multi(sha, cx),
            ClickKind::Plain => {
                self.history.multi.clear();
                self.select_commit(ix, cx);
            }
        }
        cx.notify();
    }

    fn select_range(&mut self, ix: usize, cx: &mut Context<Self>) {
        let anchor = self.selected_commit.unwrap_or(ix);
        let (lo, hi) = (anchor.min(ix), anchor.max(ix));
        if self.selected_commit.is_none() {
            self.select_commit(ix, cx);
        }
        self.history.multi = self.commits[lo..=hi].iter().map(|c| c.sha.clone()).collect();
    }

    fn toggle_multi(&mut self, sha: String, cx: &mut Context<Self>) {
        if self.history.multi.len() < 2 {
            // Seed with the primary selection.
            self.history.multi = self
                .selected_commit
                .and_then(|i| self.commits.get(i))
                .map(|c| c.sha.clone())
                .into_iter()
                .collect();
        }
        if !self.history.multi.remove(&sha) {
            self.history.multi.insert(sha);
        }
        if self.history.multi.len() < 2 {
            let only = self.history.multi.drain().next();
            if let Some(ix) = only.and_then(|s| self.commits.iter().position(|c| c.sha == s)) {
                self.select_commit(ix, cx);
            }
        }
    }
}
