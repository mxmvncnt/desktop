//! Stash model operations on `RepoModel`: the git work runs on the background
//! executor like the rest of `RepoModel` (see repo.rs), and stale loads are
//! dropped by the generation counter in [`StashState`].

use std::sync::Arc;

use gd_git::{Diff, Stash};
use gpui::{AppContext, Context};

use crate::features::diff::highlight_diff;
use crate::repo::RepoModel;

impl RepoModel {
    fn current_branch(&self) -> Option<String> {
        self.status.as_ref().and_then(|s| s.branch.clone())
    }

    /// Reload the stash list in the background (called from every full refresh).
    pub fn reload_stashes(&mut self, cx: &mut Context<Self>) {
        let repo = self.repo.clone();
        cx.spawn(async move |this, cx| {
            let res = cx.background_spawn(async move { repo.stash_list() }).await;
            this.update(cx, |this, cx| {
                match res {
                    Ok(list) => {
                        if this.stashes.list != list {
                            this.stashes.list = list;
                            this.stashes_changed(cx);
                        }
                    }
                    Err(e) => log::warn!("stash list failed: {e}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// The list changed: drop a selection whose stash is gone.
    fn stashes_changed(&mut self, cx: &mut Context<Self>) {
        let still_there = self.stashes.selected_stash().is_some();
        if !still_there && self.stashes.selected.is_some() {
            self.stashes.selected = None;
            self.stashes.viewing = false;
            self.stashes.files.clear();
            self.stashes.diff = None;
            self.stashes.hl = None;
            self.stashes.task = None;
        }
        cx.notify();
    }

    pub fn toggle_stash_panel(&mut self, cx: &mut Context<Self>) {
        self.stashes.collapsed = !self.stashes.collapsed;
        cx.notify();
    }

    pub fn toggle_stash_filter(&mut self, cx: &mut Context<Self>) {
        self.stashes.all_branches = !self.stashes.all_branches;
        cx.notify();
    }

    pub fn select_stash(&mut self, sha: String, cx: &mut Context<Self>) {
        self.stashes.viewing = true;
        if self.stashes.selected.as_deref() == Some(&sha) && !self.stashes.files.is_empty() {
            cx.notify();
            return;
        }
        self.stashes.selected = Some(sha.clone());
        self.stashes.files.clear();
        self.stashes.selected_file = None;
        self.stashes.diff = None;
        self.stashes.hl = None;
        self.stashes.gen_ += 1;
        let generation = self.stashes.gen_;
        let repo = self.repo.clone();
        self.stashes.task = Some(cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move {
                    let files = repo.stash_files(&sha)?;
                    let diff = match files.first() {
                        Some(f) => Some((repo.stash_diff(&sha, f)?, f.path.clone())),
                        None => None,
                    };
                    Ok::<_, gd_git::GitError>((files, diff))
                })
                .await;
            this.update(cx, |this, cx| {
                if this.stashes.gen_ != generation {
                    return;
                }
                match res {
                    Ok((files, diff)) => {
                        this.stashes.selected_file = (!files.is_empty()).then_some(0);
                        this.stashes.files = files;
                        this.set_stash_diff(diff, cx);
                    }
                    Err(e) => this.error = Some(e.to_string()),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    pub fn select_stash_file(&mut self, ix: usize, cx: &mut Context<Self>) {
        let (Some(sha), Some(file)) = (self.stashes.selected.clone(), self.stashes.files.get(ix).cloned()) else {
            return;
        };
        self.stashes.selected_file = Some(ix);
        self.stashes.gen_ += 1;
        let generation = self.stashes.gen_;
        let repo = self.repo.clone();
        self.stashes.task = Some(cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move {
                    let path = file.path.clone();
                    repo.stash_diff(&sha, &file).map(|d| (d, path))
                })
                .await;
            this.update(cx, |this, cx| {
                if this.stashes.gen_ != generation {
                    return;
                }
                match res {
                    Ok(d) => this.set_stash_diff(Some(d), cx),
                    Err(e) => this.error = Some(e.to_string()),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn set_stash_diff(&mut self, diff: Option<(Diff, String)>, cx: &mut Context<Self>) {
        self.stashes.hl = None;
        self.stashes.hl_task = None;
        let Some((d, path)) = diff else {
            self.stashes.diff = None;
            return;
        };
        let d = Arc::new(d);
        self.stashes.diff = Some(d.clone());
        let cache = self.stashes.cache.clone();
        let for_task = d.clone();
        self.stashes.hl_task = Some(cx.spawn(async move |this, cx| {
            let hl = cx
                .background_spawn(async move { highlight_diff(&for_task, &path, &cache) })
                .await;
            this.update(cx, |this, cx| {
                if hl.is_some() && this.stashes.diff.as_ref().is_some_and(|c| Arc::ptr_eq(c, &d)) {
                    this.stashes.hl = hl;
                    cx.notify();
                }
            })
            .ok();
        }));
    }

    fn after_stash_op(&mut self) {
        self.stashes.viewing = false;
    }

    /// Stash the working changes. `paths` empty = everything.
    pub fn stash_push(&mut self, message: String, paths: Vec<String>, cx: &mut Context<Self>) {
        let n = paths.len();
        self.run(
            move |r| r.stash_push(&message, &paths),
            move |this, sha, cx| {
                this.notice = Some(if n == 0 { "Stashed all changes".into() } else { format!("Stashed {n} path{}", if n == 1 { "" } else { "s" }) });
                this.stashes.collapsed = false;
                this.stashes.selected = Some(sha);
                this.stashes.viewing = false;
                this.stashes.files.clear();
                this.stashes.diff = None;
                this.reload_stashes(cx);
            },
            cx,
        );
    }

    pub fn stash_apply(&mut self, sha: String, cx: &mut Context<Self>) {
        self.run(
            move |r| r.stash_apply(&sha),
            |this, _, _| this.notice = Some("Applied stash (kept in the list)".into()),
            cx,
        );
    }

    pub fn stash_pop(&mut self, sha: String, cx: &mut Context<Self>) {
        self.run(
            move |r| r.stash_pop(&sha),
            |this, _, cx| {
                this.notice = Some("Restored stash".into());
                this.after_stash_op();
                this.reload_stashes(cx);
            },
            cx,
        );
    }

    pub fn stash_drop(&mut self, sha: String, cx: &mut Context<Self>) {
        self.run(
            move |r| r.stash_drop(&sha),
            |this, _, cx| {
                this.notice = Some("Discarded stash".into());
                this.after_stash_op();
                this.reload_stashes(cx);
            },
            cx,
        );
    }

    pub fn stash_rename(&mut self, stash: Stash, message: String, cx: &mut Context<Self>) {
        self.run(
            move |r| r.stash_rename(&stash.sha, stash.branch.as_deref(), &message),
            |this, _, cx| this.reload_stashes(cx),
            cx,
        );
    }

    /// Switch branches, optionally stashing the working changes first
    /// (Desktop's "Leave my changes on <branch>"). If the checkout fails after
    /// stashing, the stash is popped again so nothing is left behind.
    pub fn checkout_with_stash(&mut self, branch: gd_git::Branch, stash: bool, cx: &mut Context<Self>) {
        if !stash {
            self.checkout_now(branch, cx);
            return;
        }
        let from = self.current_branch().unwrap_or_else(|| "(detached HEAD)".into());
        let message = format!("Changes left on {from} when switching to {}", branch.name);
        self.run(
            move |r| {
                let sha = r.stash_push(&message, &[])?;
                if let Err(e) = r.checkout(&branch) {
                    let _ = r.stash_pop(&sha);
                    return Err(e);
                }
                Ok(())
            },
            move |this, _, cx| {
                this.commits.clear();
                this.selected_commit = None;
                this.last_commit = None;
                this.notice = Some(format!("Stashed your changes on {from}"));
                this.reload_stashes(cx);
            },
            cx,
        );
    }
}
