//! Pull-request preview: the merge-base comparison of a head branch against a
//! base branch, and the hosting provider's "new pull request" URL.

use crate::diff::{Diff, DiffOptions, Side};
use crate::history_ops::split_remote;
use crate::stash::parse_name_status;
use crate::{FileChange, FileStatus, GitError, Repository, Result};

/// What a PR from `head` into `base` would contain (`git diff base...head`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    /// Common ancestor of base and head: the left side of every file diff.
    pub merge_base: String,
    /// Commits on head that are not on base (`base..head`).
    pub ahead: u32,
    /// Commits on base that head doesn't have yet (`head..base`).
    pub behind: u32,
    pub files: Vec<FileChange>,
}

/// Where a pull request can be opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    GitHub,
    GitLab,
    Bitbucket,
}

impl Host {
    pub fn name(self) -> &'static str {
        match self {
            Host::GitHub => "GitHub",
            Host::GitLab => "GitLab",
            Host::Bitbucket => "Bitbucket",
        }
    }
}

impl Repository {
    /// Branch a PR most likely targets: the remote's default (`origin/HEAD`),
    /// else the first of main / master / develop that exists.
    pub fn default_base_branch(&self) -> Option<String> {
        if let Ok(out) = self.git().args(["symbolic-ref", "-q", "--short", "refs/remotes/origin/HEAD"]).run() {
            let name = out.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
        ["origin/main", "origin/master", "main", "master", "origin/develop", "develop"]
            .into_iter()
            .find(|n| self.git().args(["rev-parse", "--verify", "-q"]).arg(format!("refs/remotes/{n}")).run().is_ok()
                || self.git().args(["rev-parse", "--verify", "-q"]).arg(format!("refs/heads/{n}")).run().is_ok())
            .map(String::from)
    }

    /// Commits, behind count and file list of `base...head`.
    pub fn compare(&self, base: &str, head: &str) -> Result<Comparison> {
        let merge_base = self
            .git()
            .args(["merge-base", base, head])
            .run()
            .map_err(|_| GitError::Io(format!("{head} and {base} have no common history")))?
            .trim()
            .to_string();
        let count = |range: String| -> Result<u32> {
            Ok(self.git().args(["rev-list", "--count"]).arg(range).run()?.trim().parse().unwrap_or(0))
        };
        let ahead = count(format!("{base}..{head}"))?;
        let behind = count(format!("{head}..{base}"))?;
        let out = self
            .git()
            .args(["diff", "--name-status", "-z", "-M", "--no-ext-diff"])
            .arg(&merge_base)
            .arg(head)
            .run()?;
        Ok(Comparison { merge_base, ahead, behind, files: parse_name_status(&out) })
    }

    /// Diff of one file of a [`Comparison`] (`merge_base` -> `head`).
    pub fn compare_file_diff(&self, merge_base: &str, head: &str, file: &FileChange, opts: DiffOptions) -> Result<Diff> {
        let old = if file.status == FileStatus::New {
            Side::Empty
        } else {
            Side::Blob { rev: merge_base.to_string(), path: file.old_path.clone().unwrap_or_else(|| file.path.clone()) }
        };
        let new = if file.status == FileStatus::Deleted {
            Side::Empty
        } else {
            Side::Blob { rev: head.to_string(), path: file.path.clone() }
        };
        self.diff_sides(&old, &new, opts)
    }
}

/// Which provider a remote URL belongs to.
pub fn host_of(remote: &str) -> Option<Host> {
    let (host, _) = split_remote(remote)?;
    if host.contains("github") {
        Some(Host::GitHub)
    } else if host.contains("gitlab") {
        Some(Host::GitLab)
    } else if host.contains("bitbucket") {
        Some(Host::Bitbucket)
    } else {
        None
    }
}

/// Browser URL that opens a new pull / merge request. `base` and `head` are
/// branch names on the remote (no `origin/` prefix). `None` for unknown hosts.
pub fn compare_url(remote: &str, base: &str, head: &str) -> Option<String> {
    let (host, path) = split_remote(remote)?;
    Some(match host_of(remote)? {
        Host::GitHub => format!("https://{host}/{path}/compare/{}...{}?expand=1", enc(base, true), enc(head, true)),
        Host::GitLab => format!(
            "https://{host}/{path}/-/merge_requests/new?merge_request%5Bsource_branch%5D={}&merge_request%5Btarget_branch%5D={}",
            enc(head, false),
            enc(base, false)
        ),
        Host::Bitbucket => {
            format!("https://{host}/{path}/pull-requests/new?source={}&dest={}", enc(head, false), enc(base, false))
        }
    })
}

/// Percent-encode a branch name; `keep_slash` for use in a URL path.
fn enc(s: &str, keep_slash: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => out.push(b as char),
            b'/' if keep_slash => out.push('/'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert_eq!(
            compare_url("git@github.com:o/r.git", "main", "feat/x").unwrap(),
            "https://github.com/o/r/compare/main...feat/x?expand=1"
        );
        assert!(compare_url("https://gitlab.com/o/r.git", "main", "a b").unwrap().ends_with("target_branch%5D=main"));
        assert!(compare_url("https://example.com/o/r.git", "main", "x").is_none());
    }
}
