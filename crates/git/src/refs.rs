//! Branch listing via `git for-each-ref`.

use crate::{Repository, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BranchKind {
    Local,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Branch {
    /// Short name: `main` or `origin/main`.
    pub name: String,
    pub kind: BranchKind,
    pub is_head: bool,
    pub upstream: Option<String>,
    /// Committer date of the tip, unix seconds (for "recent" sorting).
    pub tip_time: i64,
}

impl Repository {
    /// Local branches first (most recently committed first), then remotes.
    pub fn branches(&self) -> Result<Vec<Branch>> {
        let out = self
            .git()
            .args([
                "for-each-ref",
                "--sort=-committerdate",
                "--format=%(refname)%00%(refname:short)%00%(HEAD)%00%(upstream:short)%00%(committerdate:unix)",
                "refs/heads",
                "refs/remotes",
            ])
            .run()?;
        let mut branches = Vec::new();
        for line in out.lines() {
            let f: Vec<&str> = line.split('\0').collect();
            if f.len() < 5 {
                continue;
            }
            let kind = if f[0].starts_with("refs/heads/") {
                BranchKind::Local
            } else {
                BranchKind::Remote
            };
            // Skip symbolic `origin/HEAD`.
            if kind == BranchKind::Remote && f[0].ends_with("/HEAD") {
                continue;
            }
            branches.push(Branch {
                name: f[1].to_string(),
                kind,
                is_head: f[2] == "*",
                upstream: (!f[3].is_empty()).then(|| f[3].to_string()),
                tip_time: f[4].parse().unwrap_or(0),
            });
        }
        branches.sort_by_key(|b| b.kind == BranchKind::Remote);
        Ok(branches)
    }
}
