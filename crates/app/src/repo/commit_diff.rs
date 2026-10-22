//! Commit file list + diff of the selected commit file. `commit_gen` drops
//! results of older loads (commit or file clicked again meanwhile).

use std::sync::Arc;

use gd_git::Diff;
use gpui::{AppContext, Context};

use super::state::RepoModel;
use crate::features::diff::ExpandTarget;

impl RepoModel {
    pub fn select_commit(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.history.multi.clear();
        if self.selected_commit == Some(ix) {
            return;
        }
        let Some(commit) = self.commits.get(ix) else {
            return;
        };
        self.selected_commit = Some(ix);
        self.commit_gen += 1;
        let generation = self.commit_gen;
        let sha = commit.sha.clone();
        let repo = self.repo.clone();
        let opts = self.diff_options;
        self._commit_task = Some(cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move {
                    let files = repo.commit_files(&sha)?;
                    let diff = match files.first() {
                        Some(f) => Some((repo.commit_diff(&sha, f, opts)?, f.path.clone())),
                        None => None,
                    };
                    Ok::<_, gd_git::GitError>((files, diff))
                })
                .await;
            this.update(cx, |this, cx| {
                if this.commit_gen != generation {
                    return;
                }
                match res {
                    Ok((files, diff)) => {
                        this.selected_commit_file = (!files.is_empty()).then_some(0);
                        this.commit_files = files;
                        this.set_commit_diff(diff, cx);
                    }
                    Err(e) => this.error = Some(e.to_string()),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    pub fn select_commit_file(&mut self, ix: usize, cx: &mut Context<Self>) {
        let (Some(commit), Some(file)) = (
            self.selected_commit.and_then(|c| self.commits.get(c)),
            self.commit_files.get(ix),
        ) else {
            return;
        };
        self.selected_commit_file = Some(ix);
        self.commit_gen += 1;
        let generation = self.commit_gen;
        let (sha, file, repo) = (commit.sha.clone(), file.clone(), self.repo.clone());
        let opts = self.diff_options;
        self._commit_task = Some(cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move {
                    let path = file.path.clone();
                    repo.commit_diff(&sha, &file, opts).map(|d| (d, path))
                })
                .await;
            this.update(cx, |this, cx| {
                if this.commit_gen != generation {
                    return;
                }
                match res {
                    Ok(d) => this.set_commit_diff(Some(d), cx),
                    Err(e) => this.error = Some(e.to_string()),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn set_commit_diff(&mut self, diff: Option<(Diff, String)>, cx: &mut Context<Self>) {
        self.commit_hl = None;
        self._commit_hl_task = None;
        match diff {
            Some((d, path)) => {
                let d = Arc::new(d);
                self.commit_diff = Some(d.clone());
                self._commit_hl_task = Some(self.spawn_highlight(d, path, ExpandTarget::Commit, cx));
            }
            None => self.commit_diff = None,
        }
    }
}
