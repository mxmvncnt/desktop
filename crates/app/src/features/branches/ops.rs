//! Branch rename / delete / create ops on [`RepoModel`], run through
//! [`RepoModel::run_op`] (background thread, busy flag, refresh).

use gd_git::{Branch, BranchKind};
use gpui::Context;

use super::state::RemoteBranch;
use crate::repo::{RepoEvent, RepoModel};

impl RepoModel {
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

    /// Delete a local branch (force) and/or `remote` on the server.
    pub fn delete_branch(&mut self, branch: Branch, remote: Option<RemoteBranch>, cx: &mut Context<Self>) {
        let name = branch.name.clone();
        let local = branch.kind == BranchKind::Local;
        self.run_op(
            "Deleting…",
            move |r| {
                if local {
                    r.delete_branch(&branch.name, true)?;
                }
                if let Some(RemoteBranch { remote, branch }) = &remote {
                    r.delete_remote_branch(remote, branch)?;
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

    /// Switch branch. With uncommitted changes the workspace first asks
    /// whether to stash them (Desktop's dialog).
    pub fn checkout(&mut self, branch: Branch, cx: &mut Context<Self>) {
        if !self.changes().is_empty() {
            cx.emit(RepoEvent::SwitchBranchWithChanges(branch));
            return;
        }
        self.checkout_now(branch, cx);
    }

    pub fn checkout_now(&mut self, branch: Branch, cx: &mut Context<Self>) {
        self.run(
            move |r| r.checkout(&branch),
            |this, _, _| {
                this.commits.clear();
                this.selected_commit = None;
                this.last_commit = None;
            },
            cx,
        );
    }

    pub fn create_branch(&mut self, name: String, cx: &mut Context<Self>) {
        self.run(move |r| r.create_branch(&name), |_, _, _| {}, cx);
    }
}
