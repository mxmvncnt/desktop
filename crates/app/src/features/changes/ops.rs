//! RepoModel operations of the Changes tab: what is included in the next
//! commit, committing, ignoring, discarding and undoing the last commit.

use gd_git::{FileChange, FileStatus};
use gpui::Context;

use crate::repo::{LastCommit, RepoEvent, RepoModel};

impl RepoModel {
    pub fn included_paths(&self) -> Vec<String> {
        self.changes()
            .iter()
            .filter(|c| !self.excluded.contains(&c.path) && !self.partial.contains_key(&c.path))
            .map(|c| c.path.clone())
            .collect()
    }

    /// Name used when the summary is left empty and exactly one file is
    /// selected: "Create x" / "Update x" / "Delete x" / "Rename a to b".
    pub fn default_summary(&self) -> Option<String> {
        let mut selected = self.changes().iter().filter(|c| !self.excluded.contains(&c.path));
        let c = selected.next()?;
        if selected.next().is_some() {
            return None;
        }
        let name = |p: &str| p.rsplit('/').next().unwrap_or(p).to_string();
        Some(match (c.status, &c.old_path) {
            (FileStatus::New | FileStatus::Untracked, _) => format!("Create {}", name(&c.path)),
            (FileStatus::Deleted, _) => format!("Delete {}", name(&c.path)),
            (FileStatus::Renamed, Some(old)) => format!("Rename {} to {}", name(old), name(&c.path)),
            _ => format!("Update {}", name(&c.path)),
        })
    }

    /// File checkbox: a partially selected file becomes fully included.
    pub fn toggle_included(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.partial.remove(path).is_some() {
            self.excluded.remove(path);
        } else if !self.excluded.remove(path) {
            self.excluded.insert(path.to_string());
        }
        cx.notify();
    }

    pub fn set_all_included(&mut self, included: bool, cx: &mut Context<Self>) {
        self.partial.clear();
        if included {
            self.excluded.clear();
        } else {
            self.excluded = self.changes().iter().map(|c| c.path.clone()).collect();
        }
        cx.notify();
    }

    pub fn commit(&mut self, summary: String, description: String, cx: &mut Context<Self>) {
        let paths = self.included_paths();
        let partial = self.partial_files();
        let n = paths.len() + partial.len();
        let branch = self.branch_name();
        self.committing = true;
        cx.notify();
        if self.merge.in_progress.as_ref().is_some_and(|p| p.commit_finishes()) {
            return self.commit_merge(summary, description, cx);
        }
        let message = (summary.clone(), description.clone());
        self.run(
            move |r| r.commit_partial(&summary, &description, &paths, &partial),
            move |this, _sha, _| {
                this.last_commit = Some(LastCommit {
                    summary: message.0,
                    description: message.1,
                    branch,
                    time: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0),
                });
                this.notice = Some(format!(
                    "Committed {n} file{}",
                    if n == 1 { "" } else { "s" }
                ));
                this.selected_change = None;
                // Row indices refer to diffs that no longer exist.
                this.partial.clear();
            },
            cx,
        );
    }

    /// Append gitignore `patterns` to `.gitignore`, then refresh.
    pub fn ignore(&mut self, patterns: Vec<String>, notice: String, cx: &mut Context<Self>) {
        self.run(
            move |r| r.ignore(&patterns),
            move |this, _, _| this.notice = Some(notice),
            cx,
        );
    }

    pub fn discard(&mut self, changes: Vec<FileChange>, cx: &mut Context<Self>) {
        self.run(move |r| r.discard(&changes), |_, _, _| {}, cx);
    }

    /// Soft-reset HEAD~1 and hand the message back via [`RepoEvent::CommitUndone`].
    pub fn undo_last_commit(&mut self, cx: &mut Context<Self>) {
        let last = self.last_commit.take();
        self.run(
            |r| r.undo_last_commit(),
            move |this, _, cx| {
                this.commits.clear();
                this.selected_commit = None;
                if let Some(c) = last {
                    cx.emit(RepoEvent::CommitUndone {
                        summary: c.summary,
                        description: c.description,
                    });
                }
            },
            cx,
        );
    }
}
