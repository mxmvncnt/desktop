//! Paged commit log and tab switching.

use gpui::{AppContext, Context};

use super::state::{RepoModel, Tab};

/// History is paged in so huge repos open instantly.
const HISTORY_PAGE: usize = 200;

impl RepoModel {
    pub fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        if tab == Tab::History && self.commits.is_empty() {
            self.reload_history(cx);
        }
        cx.notify();
    }

    /// Reload the pages already loaded, keeping the selected commit if it
    /// is still there.
    pub(super) fn reload_history(&mut self, cx: &mut Context<Self>) {
        let repo = self.repo.clone();
        let count = self.commits.len().max(HISTORY_PAGE);
        self.history_loading = true;
        cx.spawn(async move |this, cx| {
            let res = cx.background_spawn(async move { repo.log(0, count) }).await;
            this.update(cx, |this, cx| {
                this.history_loading = false;
                match res {
                    Ok(commits) => this.apply_reloaded_log(commits, count, cx),
                    Err(e) => this.error = Some(e.to_string()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn apply_reloaded_log(&mut self, commits: Vec<gd_git::Commit>, count: usize, cx: &mut Context<Self>) {
        let selected_sha = self.selected_commit.and_then(|ix| self.commits.get(ix)).map(|c| c.sha.clone());
        self.history_exhausted = commits.len() < count;
        self.commits = commits;
        let ix = selected_sha
            .and_then(|s| self.commits.iter().position(|c| c.sha == s))
            .or(if self.commits.is_empty() { None } else { Some(0) });
        if ix != self.selected_commit || self.commit_files.is_empty() {
            self.selected_commit = None;
            if let Some(ix) = ix {
                self.select_commit(ix, cx);
            } else {
                self.commit_files.clear();
                self.commit_diff = None;
                self.commit_hl = None;
            }
        }
    }

    /// Called by the history list when the user scrolls near the end.
    pub fn load_more_history(&mut self, cx: &mut Context<Self>) {
        if self.history_loading || self.history_exhausted {
            return;
        }
        self.history_loading = true;
        let repo = self.repo.clone();
        let skip = self.commits.len();
        cx.spawn(async move |this, cx| {
            let res = cx.background_spawn(async move { repo.log(skip, HISTORY_PAGE) }).await;
            this.update(cx, |this, cx| {
                this.history_loading = false;
                if let Ok(mut more) = res {
                    this.history_exhausted = more.len() < HISTORY_PAGE;
                    if this.commits.len() == skip {
                        this.commits.append(&mut more);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
