//! History operations on `RepoModel`: push status, tags, and the commit
//! context-menu actions (reset, checkout, revert, cherry-pick, squash,
//! reorder, amend, tags, undo).
//!
//! All git work goes through [`RepoModel::hist_run`], which runs off-thread,
//! reports errors via `RepoModel::error`, and refreshes status + history +
//! push state afterwards. Operations that stop on conflicts leave the git
//! operation in progress (see `Repository::history_op_in_progress`).

use gd_git::{Repository, ResetMode};
use gpui::{AppContext, Context};

use crate::repo::{LastCommit, RepoModel};

impl RepoModel {
    /// Recompute unpushed commits, tags, web URL and the in-progress state in
    /// the background. Called from every full refresh (so after push, fetch,
    /// commit, checkout...).
    pub fn reload_push_state(&mut self, cx: &mut Context<Self>) {
        self.history.load_gen += 1;
        let generation = self.history.load_gen;
        let repo = self.repo.clone();
        self.history.task = Some(cx.spawn(async move |this, cx| {
            let (unpushed, tags, url, no_remote) = cx
                .background_spawn(async move {
                    (
                        repo.unpushed_commits(),
                        repo.tags_by_commit(),
                        repo.github_web_url(),
                        repo.remotes().map(|r| r.is_empty()).unwrap_or(false),
                    )
                })
                .await;
            this.update(cx, |this, cx| {
                let h = &mut this.history;
                if h.load_gen != generation {
                    return;
                }
                h.all_local = no_remote;
                if let Ok(u) = unpushed {
                    h.unpushed = u;
                    h.push_state_loaded = true;
                }
                if let Ok(t) = tags {
                    h.tags = t;
                }
                h.web_url = url;
                cx.notify();
            })
            .ok();
        }));
    }

    /// Why history operations are unavailable right now (shown as disabled menu items).
    pub fn history_blocked(&self) -> bool {
        self.merge.busy.is_some() || self.merge.in_progress.is_some() || self.remote_op.is_some() || self.committing
    }

    pub fn is_unpushed(&self, sha: &str) -> bool {
        self.history.all_local || self.history.unpushed.contains(sha)
    }

    /// Run a history operation through the shared `run_op` (one at a time,
    /// refreshes afterwards; conflicts surface as the in-progress banner).
    fn hist_run<R: Send + 'static>(
        &mut self,
        label: &'static str,
        op: impl FnOnce(&Repository) -> gd_git::Result<R> + Send + 'static,
        on_ok: impl FnOnce(&mut Self, R, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        self.history.multi.clear();
        self.run_op(label, op, on_ok, cx);
    }

    pub fn reset_to_commit(&mut self, sha: String, mode: ResetMode, cx: &mut Context<Self>) {
        self.last_commit = None;
        self.hist_run(
            "Resetting",
            move |r| r.reset_to_commit(&sha, mode),
            |this, _, _| this.notice = Some("Reset to commit".into()),
            cx,
        );
    }

    pub fn checkout_commit(&mut self, sha: String, cx: &mut Context<Self>) {
        self.hist_run(
            "Checking out",
            move |r| r.checkout_commit(&sha),
            |this, _, _| {
                this.last_commit = None;
                this.notice = Some("HEAD is now detached at that commit".into())
            },
            cx,
        );
    }

    pub fn revert_commit(&mut self, sha: String, cx: &mut Context<Self>) {
        self.hist_run(
            "Reverting",
            move |r| r.revert_commit(&sha),
            |this, o, _| this.notice_for_outcome(o, "Reverted commit".into(), "Nothing to revert".into()),
            cx,
        );
    }

    pub fn create_branch_at(&mut self, name: String, sha: String, cx: &mut Context<Self>) {
        self.hist_run(
            "Creating branch",
            move |r| r.create_branch_at(&name, &sha),
            |this, _, _| this.last_commit = None,
            cx,
        );
    }

    pub fn create_tag(&mut self, name: String, sha: String, cx: &mut Context<Self>) {
        let n = name.clone();
        self.hist_run(
            "Tagging",
            move |r| r.create_tag(&name, &sha),
            move |this, _, _| this.notice = Some(format!("Created tag {n}")),
            cx,
        );
    }

    pub fn delete_tag(&mut self, name: String, cx: &mut Context<Self>) {
        let n = name.clone();
        self.hist_run(
            "Deleting tag",
            move |r| r.delete_tag(&name),
            move |this, _, _| this.notice = Some(format!("Deleted tag {n}")),
            cx,
        );
    }

    /// `shas` must be oldest first.
    pub fn cherry_pick(&mut self, shas: Vec<String>, target: String, cx: &mut Context<Self>) {
        let n = shas.len();
        self.hist_run(
            "Cherry-picking",
            move |r| r.cherry_pick(&shas, &target),
            move |this, o, _| {
                this.last_commit = None;
                let done = format!("Cherry-picked {n} commit{}", crate::workspace::plural(n as u32));
                this.notice_for_outcome(o, done, "Nothing to cherry-pick".into())
            },
            cx,
        );
    }

    pub fn squash(&mut self, shas: Vec<String>, message: String, cx: &mut Context<Self>) {
        let n = shas.len();
        self.last_commit = None;
        self.hist_run(
            "Squashing",
            move |r| r.squash_commits(&shas, &message),
            move |this, o, _| this.notice_for_outcome(o, format!("Squashed {n} commits"), "Nothing to squash".into()),
            cx,
        );
    }

    pub fn move_commit(&mut self, sha: String, up: bool, cx: &mut Context<Self>) {
        self.last_commit = None;
        self.hist_run(
            "Reordering",
            move |r| r.move_commit(&sha, up),
            |this, o, _| this.notice_for_outcome(o, "Reordered commits".into(), "Nothing to reorder".into()),
            cx,
        );
    }

    /// `commit --amend` with the staged selection and the given message.
    pub fn amend(&mut self, summary: String, description: String, cx: &mut Context<Self>) {
        let paths = self.included_paths();
        let partial = self.partial_files();
        self.committing = true;
        self.last_commit = None;
        self.hist_run(
            "Amending",
            move |r| r.amend_commit(&summary, &description, &paths, &partial),
            |this, _, _| {
                this.notice = Some("Amended last commit".into());
                this.partial.clear();
                this.selected_change = None;
            },
            cx,
        );
    }

    /// Undo the HEAD commit (soft reset), putting its message back in the
    /// commit box through the existing `CommitUndone` event.
    pub fn undo_head_commit(&mut self, cx: &mut Context<Self>) {
        let Some(head) = self.commits.first().cloned() else { return };
        self.last_commit = Some(LastCommit {
            summary: head.summary,
            description: head.body,
            branch: self.branch_name(),
            time: head.time,
        });
        self.undo_last_commit(cx);
    }
}
