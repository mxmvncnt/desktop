//! Merge / rebase / conflict operations on [`RepoModel`]. All git work runs on
//! the background executor.
//!
//! Other features (revert, cherry-pick, squash from History, branch ops) get
//! the banner, conflict list and Continue/Abort for free: start the git command
//! inside a [`RepoModel::run_op`] closure and, if it pauses, `merge.in_progress`
//! is detected on the next refresh. Use [`RepoModel::notice_for_outcome`] for
//! the message.

use gd_git::{FileStatus, OpOutcome, Repository};
use gpui::{AppContext, Context};

use super::state::{MergeMode, Resolve};
use crate::repo::{RepoEvent, RepoModel};

impl MergeMode {
    fn busy_label(self) -> &'static str {
        match self {
            MergeMode::Merge => "Merging…",
            MergeMode::Squash => "Squashing…",
            MergeMode::Rebase => "Rebasing…",
        }
    }

    /// Toast texts: (done, already up to date).
    fn notices(self, current: &str, branch: &str) -> (String, String) {
        match self {
            MergeMode::Merge => (
                format!("Merged {branch} into {current}"),
                format!("{current} is already up to date with {branch}"),
            ),
            MergeMode::Squash => (
                format!("Squashed {branch} into {current}"),
                format!("{current} is already up to date with {branch}"),
            ),
            MergeMode::Rebase => (
                format!("Rebased {current} onto {branch}"),
                format!("{current} is already based on {branch}"),
            ),
        }
    }

    fn run(self, r: &Repository, branch: &str) -> gd_git::Result<OpOutcome> {
        match self {
            MergeMode::Merge => r.merge(branch),
            MergeMode::Squash => r.merge_squash(branch),
            MergeMode::Rebase => r.rebase(branch),
        }
    }
}

impl RepoModel {
    pub fn conflict_count(&self) -> usize {
        self.changes().iter().filter(|c| c.status == FileStatus::Conflicted).count()
    }

    /// Run a blocking operation off-thread with the merge `busy` flag set,
    /// then refresh. `done` runs on the UI thread with the result (success or error).
    pub fn run_op<R: Send + 'static>(
        &mut self,
        label: &'static str,
        op: impl FnOnce(&Repository) -> gd_git::Result<R> + Send + 'static,
        done: impl FnOnce(&mut Self, R, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.merge.busy.is_some() {
            return;
        }
        self.merge.busy = Some(label);
        self.error = None;
        let repo = self.repo.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let res = cx.background_spawn(async move { op(&repo) }).await;
            this.update(cx, |this, cx| {
                this.merge.busy = None;
                this.committing = false;
                match res {
                    Ok(r) => done(this, r, cx),
                    Err(e) => this.error = Some(e.to_string()),
                }
                this.refresh(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Toast text for an operation that can pause on conflicts.
    pub fn notice_for_outcome(&mut self, outcome: OpOutcome, done: String, up_to_date: String) {
        self.notice = Some(match outcome {
            OpOutcome::Done => done,
            OpOutcome::UpToDate => up_to_date,
            OpOutcome::Conflicts => "Conflicts found: resolve them, then continue".into(),
        });
    }

    /// The history list is stale after the branch moved.
    fn reset_history_selection(&mut self) {
        self.commits.clear();
        self.selected_commit = None;
    }

    pub fn start_merge(&mut self, mode: MergeMode, branch: String, cx: &mut Context<Self>) {
        let (done, up_to_date) = mode.notices(&self.branch_name(), &branch);
        self.run_op(
            mode.busy_label(),
            move |r| mode.run(r, &branch),
            move |this, outcome, _| {
                this.notice_for_outcome(outcome, done, up_to_date);
                this.reset_history_selection();
            },
            cx,
        );
    }

    /// Banner "Continue".
    pub fn continue_operation(&mut self, cx: &mut Context<Self>) {
        let Some(op) = self.merge.in_progress.clone() else { return };
        let verb = op.verb();
        self.run_op(
            "Continuing…",
            move |r| r.continue_operation(&op),
            move |this, outcome, _| {
                this.notice_for_outcome(outcome, format!("{verb} finished"), format!("{verb} finished"));
                this.reset_history_selection();
            },
            cx,
        );
    }

    /// Banner "Abort".
    pub fn abort_operation(&mut self, cx: &mut Context<Self>) {
        let Some(op) = self.merge.in_progress.clone() else { return };
        self.run_op(
            "Aborting…",
            move |r| r.abort_operation(&op),
            |this, _, _| this.notice = Some("Operation aborted".into()),
            cx,
        );
    }

    /// Resolve one conflicted file (use ours / theirs / keep the edited file).
    pub fn resolve_file(&mut self, path: String, how: Resolve, cx: &mut Context<Self>) {
        let p = path.clone();
        self.run_op(
            "Resolving…",
            move |r| match how {
                Resolve::Ours => r.resolve_with(&p, false),
                Resolve::Theirs => r.resolve_with(&p, true),
                Resolve::Marked => r.mark_resolved(&p),
            },
            move |this, _, _| this.notice = Some(format!("Resolved {path}")),
            cx,
        );
    }

    /// Commit while a merge-like operation is paused: keeps the merge result in
    /// the index (the normal commit path resets it).
    pub fn commit_merge(&mut self, summary: String, description: String, cx: &mut Context<Self>) {
        if self.merge.busy.is_some() {
            return;
        }
        let paths: Vec<String> = self.changes().iter().map(|c| c.path.clone()).collect();
        self.committing = true;
        cx.notify();
        self.run_op(
            "Committing…",
            move |r| r.commit_merge(&summary, &description, &paths),
            |this, _, _| {
                this.committing = false;
                this.notice = Some("Merge committed".into());
                this.selected_change = None;
            },
            cx,
        );
    }
}

/// Emit the prepared message when an operation newly pauses.
pub(crate) fn message_event(summary: String, description: String) -> RepoEvent {
    RepoEvent::PreparedMessage { summary, description }
}
