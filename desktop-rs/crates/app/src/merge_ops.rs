//! Merge / rebase / conflict operations on [`RepoModel`], plus the single
//! `RepoModel.merge` state field. All git work runs on the background executor.
//!
//! Other features (revert, cherry-pick, squash from History) get the banner,
//! conflict list and Continue/Abort for free: start the git command inside a
//! [`RepoModel::run_op`] closure and, if it pauses, `merge.in_progress` is
//! detected on the next refresh. Use [`RepoModel::notice_for_outcome`] for the
//! message.

use std::collections::HashSet;

use gd_git::{Branch, FileStatus, InProgress, OpOutcome, Repository};
use gpui::{AppContext, Context};

use crate::repo::{RepoEvent, RepoModel};

/// Which flavor of "merge into current branch" to run.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MergeMode {
    Merge,
    Squash,
    Rebase,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resolve {
    Ours,
    Theirs,
    /// Keep the working-tree contents (after editing by hand).
    Marked,
}

/// State added to `RepoModel` by the merge/conflict feature.
#[derive(Default)]
pub struct MergeState {
    /// Paused operation detected on the last refresh.
    pub in_progress: Option<InProgress>,
    /// A merge/continue/abort is running (disables the banner buttons).
    pub busy: Option<&'static str>,
}

impl RepoModel {
    pub fn conflicted_paths(&self) -> HashSet<&str> {
        self.changes()
            .iter()
            .filter(|c| c.status == FileStatus::Conflicted)
            .map(|c| c.path.as_str())
            .collect()
    }

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

    pub fn start_merge(&mut self, mode: MergeMode, branch: String, cx: &mut Context<Self>) {
        let current = self.branch_name();
        let name = branch.clone();
        self.run_op(
            match mode {
                MergeMode::Merge => "Merging…",
                MergeMode::Squash => "Squashing…",
                MergeMode::Rebase => "Rebasing…",
            },
            move |r| match mode {
                MergeMode::Merge => r.merge(&name),
                MergeMode::Squash => r.merge_squash(&name),
                MergeMode::Rebase => r.rebase(&name),
            },
            move |this, outcome, _| {
                let (done, up) = match mode {
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
                };
                this.notice_for_outcome(outcome, done, up);
                this.commits.clear();
                this.selected_commit = None;
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
                this.notice_for_outcome(
                    outcome,
                    format!("{verb} finished"),
                    format!("{verb} finished"),
                );
                this.commits.clear();
                this.selected_commit = None;
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

    pub fn rename_branch(&mut self, old: String, new: String, cx: &mut Context<Self>) {
        self.run_op(
            "Renaming…",
            {
                let (o, n) = (old.clone(), new.clone());
                move |r| r.rename_branch(&o, &n)
            },
            move |this, _, _| this.notice = Some(format!("Renamed {old} to {new}")),
            cx,
        );
    }

    /// Delete a local (force) or remote branch; `remote` = also delete this
    /// `(remote, branch)` on the server.
    pub fn delete_branch(
        &mut self,
        branch: Branch,
        remote: Option<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        let name = branch.name.clone();
        let local = branch.kind == gd_git::BranchKind::Local;
        self.run_op(
            "Deleting…",
            move |r| {
                if local {
                    r.delete_branch(&branch.name, true)?;
                }
                if let Some((remote, name)) = &remote {
                    r.delete_remote_branch(remote, name)?;
                }
                Ok(())
            },
            move |this, _, _| this.notice = Some(format!("Deleted {name}")),
            cx,
        );
    }

    pub fn create_branch_from(&mut self, name: String, base: String, cx: &mut Context<Self>) {
        self.run_op(
            "Creating…",
            {
                let (n, b) = (name.clone(), base.clone());
                move |r| r.create_branch_from(&n, &b)
            },
            move |this, _, _| {
                this.commits.clear();
                this.selected_commit = None;
                this.notice = Some(format!("Created {name} from {base}"));
            },
            cx,
        );
    }
}

/// Emit the prepared message when an operation newly pauses.
pub(crate) fn message_event(summary: String, description: String) -> RepoEvent {
    RepoEvent::PreparedMessage { summary, description }
}
