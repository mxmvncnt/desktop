//! Merge / squash / rebase into the current branch, conflict resolution, and
//! branch management used by the branch context menu.

use crate::progress::OpOutcome;
use crate::{GitError, Repository, Result};

/// What merging (or rebasing onto) `branch` would do, computed without
/// touching the working tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergePreview {
    pub branch: String,
    /// Commits reachable from `branch` but not from HEAD (`HEAD..branch`).
    pub ahead: u32,
    /// Commits reachable from HEAD but not from `branch` (`branch..HEAD`).
    pub behind: u32,
    /// Files that would conflict, when both sides diverged and
    /// `git merge-tree --write-tree` is available.
    pub conflicts: Option<Vec<String>>,
}

/// Default subject of a squash-merge commit.
pub fn default_squash_summary(branch: &str, current: &str) -> String {
    format!("Squash merge {branch} into {current}")
}

impl Repository {
    fn count(&self, range: &str) -> Result<u32> {
        let out = self.git().args(["rev-list", "--count", range, "--"]).run()?;
        out.trim().parse().map_err(|_| GitError::Parse(out))
    }

    /// Number of commits `branch` has that HEAD lacks (`git rev-list --count HEAD..branch`).
    pub fn commits_ahead(&self, branch: &str) -> Result<u32> {
        self.count(&format!("HEAD..{branch}"))
    }

    pub fn merge_preview(&self, branch: &str) -> Result<MergePreview> {
        let ahead = self.commits_ahead(branch)?;
        let behind = self.count(&format!("{branch}..HEAD"))?;
        let conflicts = if ahead > 0 && behind > 0 {
            self.git()
                .args(["merge-tree", "--write-tree", "--name-only", "--no-messages", "HEAD", branch])
                .allow_exit_one()
                .run()
                .ok()
                .map(|out| {
                    out.lines()
                        .skip(1)
                        .take_while(|l| !l.is_empty())
                        .map(String::from)
                        .collect()
                })
        } else {
            Some(Vec::new())
        };
        Ok(MergePreview { branch: branch.to_string(), ahead, behind, conflicts })
    }

    fn current_branch_name(&self) -> String {
        self.git()
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .run()
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "HEAD".into())
    }

    /// Regular merge (`git merge --no-edit`). Conflicts leave the merge paused.
    pub fn merge(&self, branch: &str) -> Result<OpOutcome> {
        if self.commits_ahead(branch)? == 0 {
            return Ok(OpOutcome::UpToDate);
        }
        let res = self.git().no_editor().args(["merge", "--no-edit", branch]).run();
        self.outcome_of(res.map(|_| ()))
    }

    /// `git merge --squash` followed by one commit. `summary` is the commit
    /// subject (blank or `None`: "Squash merge <branch> into <current>"); the
    /// body lists the squashed commit subjects.
    /// Conflicts leave the index unmerged (reported as `InProgress::Squash`).
    pub fn merge_squash(&self, branch: &str, summary: Option<&str>) -> Result<OpOutcome> {
        if self.commits_ahead(branch)? == 0 {
            return Ok(OpOutcome::UpToDate);
        }
        let res = self.git().no_editor().args(["merge", "--squash", branch]).run();
        if self.outcome_of(res.map(|_| ()))? == OpOutcome::Conflicts {
            return Ok(OpOutcome::Conflicts);
        }
        // Nothing staged (e.g. the branch only reverted its own changes).
        if self.git().args(["diff", "--cached", "--quiet"]).run().is_ok() {
            return Ok(OpOutcome::UpToDate);
        }
        let subjects = self
            .git()
            .args(["log", "--reverse", "--format=* %s", &format!("HEAD..{branch}")])
            .run()
            .unwrap_or_default();
        let subject = match summary.map(str::trim) {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => default_squash_summary(branch, &self.current_branch_name()),
        };
        let message = format!("{subject}\n\n{}", subjects.trim());
        self.git().args(["commit", "-q", "-F", "-"]).stdin(message).run()?;
        Ok(OpOutcome::Done)
    }

    /// `git rebase <onto>` of the current branch. Conflicts leave the rebase paused.
    pub fn rebase(&self, onto: &str) -> Result<OpOutcome> {
        if self.commits_ahead(onto)? == 0 {
            return Ok(OpOutcome::UpToDate);
        }
        let res = self.git().no_editor().args(["rebase", onto]).run();
        self.outcome_of(res.map(|_| ()))
    }

    /// Commit during a merge-like operation (merge, squash, cherry-pick,
    /// revert): stage `paths` WITHOUT resetting the index (that would lose the
    /// merge result), then commit. git refuses while conflicts remain.
    pub fn commit_merge(&self, summary: &str, description: &str, paths: &[String]) -> Result<()> {
        if summary.trim().is_empty() {
            return Err(GitError::Invalid("commit summary is empty".into()));
        }
        for chunk in paths.chunks(500) {
            self.git()
                .args(["add", "-A", "--"])
                .args(chunk.iter().map(String::as_str))
                .run()?;
        }
        let mut message = summary.trim().to_string();
        if !description.trim().is_empty() {
            message.push_str("\n\n");
            message.push_str(description.trim());
        }
        self.git().no_editor().args(["commit", "-q", "-F", "-"]).stdin(message).run()?;
        Ok(())
    }

    // ---- conflict resolution --------------------------------------------

    /// Resolve `path` with our side (`ours`) or theirs, then stage it. If that
    /// side deleted the file, the file is removed instead.
    pub fn resolve_with(&self, path: &str, theirs: bool) -> Result<()> {
        let side = if theirs { "--theirs" } else { "--ours" };
        match self.git().args(["checkout", side, "--", path]).run() {
            Ok(_) => {
                self.git().args(["add", "-A", "--", path]).run()?;
            }
            Err(GitError::Failed { stderr, .. }) if stderr.contains("does not have") => {
                self.git().args(["rm", "-q", "--", path]).run()?;
            }
            Err(e) => return Err(e),
        }
        Ok(())
    }

    /// Mark `path` resolved with whatever is in the working tree.
    pub fn mark_resolved(&self, path: &str) -> Result<()> {
        self.git().args(["add", "-A", "--", path]).run()?;
        Ok(())
    }

    // ---- branch management ------------------------------------------------

    pub fn rename_branch(&self, old: &str, new: &str) -> Result<()> {
        self.git().args(["branch", "-m", old, new]).run()?;
        Ok(())
    }

    /// Delete `branch` on `remote` (`git push <remote> --delete <branch>`).
    pub fn delete_remote_branch(&self, remote: &str, branch: &str) -> Result<()> {
        self.git().args(["push", remote, "--delete", branch]).network().run()?;
        Ok(())
    }

    /// Create `name` at `base` (any branch or commit-ish) and switch to it.
    pub fn create_branch_from(&self, name: &str, base: &str) -> Result<()> {
        self.git().args(["checkout", "-q", "-b", name, base, "--no-track"]).run()?;
        Ok(())
    }
}
