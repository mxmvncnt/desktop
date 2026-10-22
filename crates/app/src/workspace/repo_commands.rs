//! `impl Workspace`: thin forwards from UI commands to the repo model.

use gpui::{Context, Window};

use super::state::Workspace;
use crate::repo::{RemoteOp, Tab};

impl Workspace {
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if let Some(repo) = &self.repo {
            repo.update(cx, |r, cx| r.refresh(cx));
        }
    }

    pub(super) fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.try_amend(window, cx) {
            return;
        }
        let Some(repo) = self.repo.clone() else { return };
        let summary = self.summary.read(cx).as_str().to_string();
        let description = self.description.read(cx).as_str().to_string();
        if summary.trim().is_empty() || !repo.read(cx).has_included() {
            return;
        }
        repo.update(cx, |r, cx| r.commit(summary, description, cx));
        self.summary.update(cx, |s, cx| s.emplace("", cx));
        self.description.update(cx, |s, cx| s.emplace("", cx));
        self.focus(window, cx);
    }

    pub fn undo_commit(&mut self, cx: &mut Context<Self>) {
        if let Some(repo) = &self.repo {
            repo.update(cx, |r, cx| r.undo_last_commit(cx));
        }
    }

    pub(crate) fn remote_action(&mut self, op: RemoteOp, cx: &mut Context<Self>) {
        if let Some(repo) = &self.repo {
            repo.update(cx, |r, cx| r.remote(op, cx));
        }
    }

    pub(super) fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        if let Some(repo) = &self.repo {
            repo.update(cx, |r, cx| r.set_tab(tab, cx));
        }
    }
}
