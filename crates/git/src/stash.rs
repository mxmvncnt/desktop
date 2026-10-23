//! Multiple stashes: list, push (all / pathspec, with untracked files),
//! apply, pop, drop, rename, and a stash's file list + per-file diffs.
//!
//! Layout of a stash (see `git help stash`): the stash commit `S` is the
//! working-tree state with parents `[HEAD-at-stash-time, index-commit]` and,
//! when `-u` was used, a third parent holding the untracked files as a
//! parentless commit. So:
//! - tracked changes = `git diff S^1 S`
//! - untracked files = the tree of `S^3` (a root commit: `git show S^3`)

use crate::diff::{Diff, Side};
use crate::{FileChange, FileStatus, GitError, Repository, Result};

const SEP: char = '\u{1f}';

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Stash {
    /// Position in `git stash list` (`stash@{index}`) at listing time. Shifts
    /// when other stashes are dropped, so write operations re-resolve by `sha`.
    pub index: usize,
    pub sha: String,
    pub short_sha: String,
    /// Stash time, unix seconds.
    pub time: i64,
    /// Local date/time as formatted by git, e.g. `2026-10-20 14:05`.
    pub date: String,
    /// Branch it was made on; `None` for a detached HEAD.
    pub branch: Option<String>,
    /// User-supplied message; empty for automatic "WIP on ..." stashes.
    pub message: String,
    /// The raw reflog subject (`WIP on main: abc123 subject` / `On main: msg`).
    pub subject: String,
}

impl Stash {
    /// What to show as the title.
    pub fn title(&self) -> String {
        if !self.message.is_empty() {
            self.message.clone()
        } else {
            match &self.branch {
                Some(b) => format!("WIP on {b}"),
                None => "WIP on (no branch)".into(),
            }
        }
    }
}

/// Split a reflog subject into `(branch, message)`.
/// `WIP on <branch>: <sha> <subject>` -> message empty; `On <branch>: <msg>` -> msg.
pub fn parse_subject(subject: &str) -> (Option<String>, String) {
    let (wip, rest) = if let Some(r) = subject.strip_prefix("WIP on ") {
        (true, r)
    } else if let Some(r) = subject.strip_prefix("On ") {
        (false, r)
    } else {
        // Created with `git stash store -m ...`: no structure to parse.
        return (None, subject.to_string());
    };
    // Branch names cannot contain ':' so the first ": " ends the branch.
    let (branch, msg) = match rest.split_once(": ") {
        Some((b, m)) => (b, m),
        None => (rest.trim_end_matches(':'), ""),
    };
    let branch = (branch != "(no branch)" && !branch.is_empty()).then(|| branch.to_string());
    (branch, if wip { String::new() } else { msg.to_string() })
}

impl Repository {
    /// All stashes, newest first.
    pub fn stash_list(&self) -> Result<Vec<Stash>> {
        let out = self
            .git()
            .args([
                "stash",
                "list",
                "-z",
                "--date=format-local:%Y-%m-%d %H:%M",
                "--format=%H%x1f%h%x1f%ct%x1f%cd%x1f%gs",
            ])
            .run()?;
        Ok(out
            .split('\0')
            .filter(|r| !r.trim().is_empty())
            .enumerate()
            .filter_map(|(index, rec)| {
                let f: Vec<&str> = rec.trim_start_matches('\n').splitn(5, SEP).collect();
                if f.len() < 5 {
                    return None;
                }
                let (branch, message) = parse_subject(f[4]);
                Some(Stash {
                    index,
                    sha: f[0].into(),
                    short_sha: f[1].into(),
                    time: f[2].parse().unwrap_or(0),
                    date: f[3].into(),
                    branch,
                    message,
                    subject: f[4].into(),
                })
            })
            .collect())
    }

    /// Current `stash@{n}` index of the stash commit `sha`.
    fn stash_index(&self, sha: &str) -> Result<usize> {
        self.stash_list()?
            .into_iter()
            .find(|s| s.sha == sha)
            .map(|s| s.index)
            .ok_or_else(|| GitError::Invalid("that stash no longer exists".into()))
    }

    /// Stash changes. `paths` empty = everything (including untracked files);
    /// otherwise only those paths (pathspecs are taken literally). Returns the
    /// new stash's sha.
    pub fn stash_push(&self, message: &str, paths: &[String]) -> Result<String> {
        if !self.has_head() {
            return Err(GitError::Invalid(
                "Can't stash before the first commit.".into(),
            ));
        }
        let before = self.stash_top();
        // `--literal-pathspecs` breaks `-u` cleanup of untracked files when no
        // pathspec is given, so only pass it for the pathspec form.
        let mut g = self.git();
        if !paths.is_empty() {
            g = g.arg("--literal-pathspecs");
        }
        let mut g = g.args(["stash", "push", "--include-untracked"]);
        if !message.trim().is_empty() {
            g = g.args(["-m", message.trim()]);
        }
        if !paths.is_empty() {
            g = g.arg("--").args(paths.iter().map(String::as_str));
        }
        g.run()?;
        match self.stash_top() {
            Some(top) if Some(&top) != before.as_ref() => Ok(top),
            _ => Err(GitError::Invalid("No local changes to stash.".into())),
        }
    }

    fn stash_top(&self) -> Option<String> {
        self.git()
            .args(["rev-parse", "--verify", "-q", "refs/stash"])
            .run()
            .ok()
            .map(|s| s.trim().to_string())
    }

    /// Apply a stash, keeping it. Conflicts leave the working tree in the
    /// conflicted state and return an error mentioning them.
    pub fn stash_apply(&self, sha: &str) -> Result<()> {
        self.git().args(["stash", "apply", sha]).run().map(|_| ()).map_err(tag_conflict)
    }

    /// Apply and drop. On conflicts git keeps the stash (so nothing is lost).
    pub fn stash_pop(&self, sha: &str) -> Result<()> {
        let ix = self.stash_index(sha)?;
        self.git()
            .args(["stash", "pop"])
            .arg(format!("stash@{{{ix}}}"))
            .run()
            .map(|_| ())
            .map_err(tag_conflict)
    }

    pub fn stash_drop(&self, sha: &str) -> Result<()> {
        let ix = self.stash_index(sha)?;
        self.git()
            .args(["stash", "drop", "--quiet"])
            .arg(format!("stash@{{{ix}}}"))
            .run()?;
        Ok(())
    }

    /// Change a stash's message. Git has no rename: re-`store` the same commit
    /// with a new reflog message, then drop the old entry. The stash moves to
    /// the top of the list and its list date becomes now (the commit date is
    /// unchanged). Returns the sha (unchanged).
    pub fn stash_rename(&self, sha: &str, branch: Option<&str>, message: &str) -> Result<()> {
        let subject = match (message.trim(), branch) {
            ("", Some(b)) => format!("WIP on {b}"),
            ("", None) => "WIP on (no branch)".to_string(),
            (m, Some(b)) => format!("On {b}: {m}"),
            (m, None) => format!("On (no branch): {m}"),
        };
        let old = self.stash_index(sha)?;
        self.git().args(["stash", "store", "-m", &subject, sha]).run()?;
        // The old entry shifted down by one after the store.
        self.git()
            .args(["stash", "drop", "--quiet"])
            .arg(format!("stash@{{{}}}", old + 1))
            .run()?;
        Ok(())
    }

    /// Files in a stash: tracked changes first, then untracked files
    /// (status [`FileStatus::Untracked`], read from the third parent).
    pub fn stash_files(&self, sha: &str) -> Result<Vec<FileChange>> {
        let out = self
            .git()
            .args(["diff", "--name-status", "-z", "-M", "--no-ext-diff"])
            .arg(format!("{sha}^1"))
            .arg(sha)
            .run()?;
        let mut files = parse_name_status(&out);
        if self.has_untracked_parent(sha) {
            let out = self
                .git()
                .args(["ls-tree", "-r", "--name-only", "-z"])
                .arg(format!("{sha}^3"))
                .run()?;
            let mut untracked: Vec<String> = out
                .split('\0')
                .filter(|p| !p.is_empty())
                .map(String::from)
                .collect();
            untracked.sort();
            files.extend(untracked.into_iter().map(|path| FileChange {
                path,
                old_path: None,
                status: FileStatus::Untracked,
            }));
        }
        Ok(files)
    }

    fn has_untracked_parent(&self, sha: &str) -> bool {
        self.git()
            .args(["rev-parse", "--verify", "-q"])
            .arg(format!("{sha}^3"))
            .run()
            .is_ok()
    }

    /// Diff of one file in a stash (tracked: base `^1` vs stash; untracked
    /// files live in the parentless `^3` commit).
    pub fn stash_diff(&self, sha: &str, file: &FileChange) -> Result<Diff> {
        let (old, new) = if file.status == FileStatus::Untracked {
            (Side::Empty, Side::Blob { rev: format!("{sha}^3"), path: file.path.clone() })
        } else {
            let old = if file.status == FileStatus::New {
                Side::Empty
            } else {
                Side::Blob {
                    rev: format!("{sha}^1"),
                    path: file.old_path.clone().unwrap_or_else(|| file.path.clone()),
                }
            };
            let new = if file.status == FileStatus::Deleted {
                Side::Empty
            } else {
                Side::Blob { rev: sha.to_string(), path: file.path.clone() }
            };
            (old, new)
        };
        self.diff_sides(&old, &new, Default::default())
    }
}

pub(crate) fn parse_name_status(out: &str) -> Vec<FileChange> {
    let mut fields = out.split('\0').filter(|s| !s.is_empty());
    let mut files = Vec::new();
    while let Some(code) = fields.next() {
        let status = match code.as_bytes().first() {
            Some(b'A') => FileStatus::New,
            Some(b'D') => FileStatus::Deleted,
            Some(b'R') => FileStatus::Renamed,
            Some(b'C') => FileStatus::Copied,
            Some(b'T') => FileStatus::TypeChanged,
            Some(b'U') => FileStatus::Conflicted,
            _ => FileStatus::Modified,
        };
        let first = fields.next().unwrap_or_default().to_string();
        let (path, old_path) = if matches!(status, FileStatus::Renamed | FileStatus::Copied) {
            (fields.next().unwrap_or_default().to_string(), Some(first))
        } else {
            (first, None)
        };
        files.push(FileChange { path, old_path, status });
    }
    files
}

/// Make conflict failures recognisable and actionable in the UI.
fn tag_conflict(e: GitError) -> GitError {
    match e {
        GitError::Failed { args, stderr } if stderr.contains("CONFLICT") => GitError::Failed {
            args,
            stderr: format!(
                "The stash was applied with conflicts. Resolve them in your working tree.\n{stderr}"
            ),
        },
        e => e,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_subject;

    #[test]
    fn subjects() {
        assert_eq!(parse_subject("WIP on main: abc123 Fix"), (Some("main".into()), String::new()));
        assert_eq!(parse_subject("On feat/x: my msg: more"), (Some("feat/x".into()), "my msg: more".into()));
        assert_eq!(parse_subject("WIP on (no branch): abc Fix"), (None, String::new()));
        assert_eq!(parse_subject("custom"), (None, "custom".into()));
    }
}
