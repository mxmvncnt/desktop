//! Detection (and generic continue/abort) of a multi-step operation that is
//! paused in the git directory: merge, squash-merge, rebase, cherry-pick, revert.
//!
//! Detection only reads the filesystem (plus one tiny `git` call to name things),
//! so the UI loads it on every status refresh. Other features (revert,
//! cherry-pick, squash from History) need no extra code to get the banner,
//! the conflict list and Continue/Abort: they only need to start their `git`
//! command and let the operation pause.


use crate::{Repository, Result};

/// What is paused in this repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InProgress {
    /// `MERGE_HEAD` exists. `branch` comes from `MERGE_MSG` ("Merge branch 'x'").
    Merge { branch: Option<String>, head: String },
    /// `git merge --squash` stopped on conflicts (`SQUASH_MSG` + unmerged paths).
    Squash,
    /// `rebase-merge/` or `rebase-apply/`.
    Rebase {
        branch: Option<String>,
        onto: Option<String>,
        /// 1-based number of the commit being applied.
        step: u32,
        total: u32,
    },
    /// `CHERRY_PICK_HEAD`.
    CherryPick { sha: String, subject: String },
    /// `REVERT_HEAD`.
    Revert { sha: String, subject: String },
}

impl InProgress {
    /// Short verb for banners: "Merging", "Rebasing", ...
    pub fn verb(&self) -> &'static str {
        match self {
            Self::Merge { .. } => "Merging",
            Self::Squash => "Squash merging",
            Self::Rebase { .. } => "Rebasing",
            Self::CherryPick { .. } => "Cherry-picking",
            Self::Revert { .. } => "Reverting",
        }
    }

    /// Whether committing from the commit box finishes the operation
    /// (merge-like) as opposed to `--continue` only (rebase).
    pub fn commit_finishes(&self) -> bool {
        !matches!(self, Self::Rebase { .. })
    }
}

/// Result of an operation that can pause on conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpOutcome {
    /// Completed (a commit was created, or the rebase finished).
    Done,
    /// Nothing to do.
    UpToDate,
    /// Paused: resolve conflicts, then continue (see [`InProgress`]).
    Conflicts,
}

fn read(path: std::path::PathBuf) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

/// `Merge branch 'feat' into main` / `Merge remote-tracking branch 'origin/x'` -> `feat`.
fn merge_branch_from_msg(msg: &str) -> Option<String> {
    let first = msg.lines().next()?;
    let start = first.find('\'')? + 1;
    let end = first[start..].find('\'')? + start;
    Some(first[start..end].to_string())
}

impl Repository {
    /// The paused operation, if any. Cheap; call from a background task.
    pub fn in_progress(&self) -> Option<InProgress> {
        let dir = self.git_dir();
        // Rebase first: a rebase step can leave CHERRY_PICK_HEAD behind.
        for sub in ["rebase-merge", "rebase-apply"] {
            let d = dir.join(sub);
            if d.is_dir() {
                let branch = read(d.join("head-name"))
                    .map(|h| h.strip_prefix("refs/heads/").unwrap_or(&h).to_string())
                    .filter(|h| h != "detached HEAD");
                let onto = read(d.join("onto")).map(|sha| self.describe_commit(&sha));
                let (step, total) = if sub == "rebase-merge" {
                    (read(d.join("msgnum")), read(d.join("end")))
                } else {
                    (read(d.join("next")), read(d.join("last")))
                };
                return Some(InProgress::Rebase {
                    branch,
                    onto,
                    step: step.and_then(|s| s.parse().ok()).unwrap_or(0),
                    total: total.and_then(|s| s.parse().ok()).unwrap_or(0),
                });
            }
        }
        if let Some(head) = read(dir.join("MERGE_HEAD")) {
            let branch = read(dir.join("MERGE_MSG")).and_then(|m| merge_branch_from_msg(&m));
            return Some(InProgress::Merge { branch, head: head.lines().next().unwrap_or("").into() });
        }
        for (file, revert) in [("CHERRY_PICK_HEAD", false), ("REVERT_HEAD", true)] {
            if let Some(sha) = read(dir.join(file)) {
                let subject = self
                    .git()
                    .args(["log", "-1", "--format=%s", &sha])
                    .run()
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default();
                let sha = sha.chars().take(7).collect();
                return Some(if revert {
                    InProgress::Revert { sha, subject }
                } else {
                    InProgress::CherryPick { sha, subject }
                });
            }
        }
        if dir.join("SQUASH_MSG").is_file() && self.has_unmerged() {
            return Some(InProgress::Squash);
        }
        None
    }

    /// Branch name for a commit if one points at it, else the short sha.
    fn describe_commit(&self, sha: &str) -> String {
        self.git()
            .args(["name-rev", "--name-only", "--no-undefined", "--refs=refs/heads/*", "--refs=refs/remotes/*", sha])
            .run()
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && !s.contains(['~', '^']))
            .unwrap_or_else(|| sha.chars().take(7).collect())
    }

    /// Whether the index has unmerged (conflicted) entries.
    pub fn has_unmerged(&self) -> bool {
        self.git()
            .args(["ls-files", "-u", "-z"])
            .run_bytes()
            .map(|o| !o.is_empty())
            .unwrap_or(false)
    }

    /// The message git prepared for the operation (`MERGE_MSG`, else
    /// `SQUASH_MSG`) as (summary, description), comment lines removed.
    pub fn prepared_message(&self) -> Option<(String, String)> {
        let dir = self.git_dir();
        let raw = std::fs::read_to_string(dir.join("MERGE_MSG"))
            .or_else(|_| std::fs::read_to_string(dir.join("SQUASH_MSG")))
            .ok()?;
        let text: String = raw
            .lines()
            .filter(|l| !l.starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        let (summary, rest) = text.split_once('\n').unwrap_or((text, ""));
        Some((summary.trim().to_string(), rest.trim().to_string()))
    }

    /// Finish the paused operation (all conflicts must be resolved and staged).
    /// Returns [`OpOutcome::Conflicts`] when a rebase / multi-pick stops again.
    pub fn continue_operation(&self, op: &InProgress) -> Result<OpOutcome> {
        let res = match op {
            InProgress::Merge { .. } => self.git().no_editor().args(["merge", "--continue"]).run(),
            InProgress::Squash => self
                .git()
                .no_editor()
                .args(["commit", "-q", "--cleanup=strip", "-F"])
                .arg(self.git_dir().join("SQUASH_MSG"))
                .run(),
            InProgress::Rebase { .. } => self.git().no_editor().args(["rebase", "--continue"]).run(),
            InProgress::CherryPick { .. } => {
                self.git().no_editor().args(["cherry-pick", "--continue"]).run()
            }
            InProgress::Revert { .. } => self.git().no_editor().args(["revert", "--continue"]).run(),
        };
        self.outcome_of(res.map(|_| ()))
    }

    /// Abort the paused operation and restore the pre-operation state.
    pub fn abort_operation(&self, op: &InProgress) -> Result<()> {
        let args: &[&str] = match op {
            InProgress::Merge { .. } => &["merge", "--abort"],
            InProgress::Squash => &["reset", "-q", "--merge"],
            InProgress::Rebase { .. } => &["rebase", "--abort"],
            InProgress::CherryPick { .. } => &["cherry-pick", "--abort"],
            InProgress::Revert { .. } => &["revert", "--abort"],
        };
        self.git().no_editor().args(args.iter().copied()).run()?;
        Ok(())
    }

    /// Map a failed git command to `Conflicts` when it merely paused.
    pub(crate) fn outcome_of(&self, res: Result<()>) -> Result<OpOutcome> {
        match res {
            Ok(()) => Ok(OpOutcome::Done),
            Err(e) => {
                if self.in_progress().is_some() && self.has_unmerged() {
                    Ok(OpOutcome::Conflicts)
                } else {
                    Err(e)
                }
            }
        }
    }
}

