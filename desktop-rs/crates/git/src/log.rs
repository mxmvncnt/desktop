//! History: commit list and per-commit file lists.

use crate::{FileChange, FileStatus, Repository, Result};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Commit {
    pub sha: String,
    pub short_sha: String,
    pub author_name: String,
    pub author_email: String,
    /// Author date, unix seconds.
    pub time: i64,
    pub summary: String,
    pub body: String,
    pub parents: Vec<String>,
}

pub type CommitFile = FileChange;

const SEP: char = '\u{1f}';

impl Repository {
    /// Commits reachable from HEAD, newest first. Paged with `skip`/`limit`
    /// so the history list can load lazily as the user scrolls.
    pub fn log(&self, skip: usize, limit: usize) -> Result<Vec<Commit>> {
        if !self.has_head() {
            return Ok(Vec::new());
        }
        let out = self
            .git()
            .args([
                "log",
                "-z",
                "--format=%H%x1f%h%x1f%an%x1f%ae%x1f%at%x1f%P%x1f%s%x1f%b",
            ])
            .arg(format!("--skip={skip}"))
            .arg(format!("--max-count={limit}"))
            .arg("HEAD")
            .run()?;
        Ok(out
            .split('\0')
            .filter(|r| !r.is_empty())
            .filter_map(|record| {
                let f: Vec<&str> = record.splitn(8, SEP).collect();
                if f.len() < 8 {
                    return None;
                }
                Some(Commit {
                    sha: f[0].trim_start_matches('\n').into(),
                    short_sha: f[1].into(),
                    author_name: f[2].into(),
                    author_email: f[3].into(),
                    time: f[4].parse().unwrap_or(0),
                    parents: f[5].split(' ').filter(|s| !s.is_empty()).map(Into::into).collect(),
                    summary: f[6].into(),
                    body: f[7].trim_end().into(),
                })
            })
            .collect())
    }

    /// Files changed by `sha` (against its first parent; root commits against
    /// the empty tree).
    pub fn commit_files(&self, sha: &str) -> Result<Vec<CommitFile>> {
        let out = self
            .git()
            .args([
                "show",
                "--format=",
                "-z",
                "--name-status",
                "-M",
                "--diff-merges=first-parent",
                sha,
            ])
            .run()?;
        let mut fields = out.split('\0').filter(|s| !s.is_empty());
        let mut files = Vec::new();
        while let Some(code) = fields.next() {
            let code = code.trim_start_matches('\n');
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
            files.push(FileChange {
                path,
                old_path,
                status,
            });
        }
        Ok(files)
    }
}
