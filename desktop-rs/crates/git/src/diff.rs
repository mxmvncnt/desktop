//! Unified-diff loading and parsing into a flat, render-ready line list.
//!
//! The UI renders diffs with a virtualized `uniform_list`, so the parsed form
//! is a flat `Vec<DiffLine>` (hunk headers included) rather than nested hunks:
//! row `i` of the list is `lines[i]`, O(1).

use crate::{FileChange, FileStatus, Repository, Result};

/// Diffs bigger than this are truncated (Desktop shows "diff too large").
const MAX_LINES: usize = 50_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Hunk,
    Context,
    Add,
    Delete,
    /// "\ No newline at end of file"
    NoNewline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    /// Line content without the leading `+`/`-`/` ` marker.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    /// Index into [`Diff::lines`] of the hunk header.
    pub header_index: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diff {
    pub lines: Vec<DiffLine>,
    pub hunks: Vec<DiffHunk>,
    pub is_binary: bool,
    pub truncated: bool,
    /// Index of the row with the most characters; the UI measures this row so
    /// the horizontal scroll extent covers the widest line, not just row 0.
    pub longest_line: usize,
    pub added: u32,
    pub deleted: u32,
}

impl Repository {
    /// Diff of a working-directory change against HEAD (index + worktree).
    pub fn working_diff(&self, change: &FileChange) -> Result<Diff> {
        let raw = match change.status {
            FileStatus::Untracked => self
                .git()
                .args(["diff", "--no-index", "--no-ext-diff", "--", "/dev/null"])
                .arg(&change.path)
                .allow_exit_one()
                .run()?,
            _ if !self.has_head() => self
                .git()
                .args(["diff", "--cached", "--no-ext-diff", "--"])
                .arg(&change.path)
                .run()?,
            _ => {
                let mut g = self
                    .git()
                    .args(["diff", "HEAD", "-M", "--no-ext-diff", "--"])
                    .arg(&change.path);
                if let Some(old) = &change.old_path {
                    g = g.arg(old);
                }
                g.run()?
            }
        };
        Ok(parse_diff(&raw))
    }

    /// Diff of one file within a commit.
    pub fn commit_diff(&self, sha: &str, file: &FileChange) -> Result<Diff> {
        let mut g = self
            .git()
            .args([
                "show",
                "--format=",
                "-M",
                "--no-ext-diff",
                "--diff-merges=first-parent",
                sha,
                "--",
            ])
            .arg(&file.path);
        if let Some(old) = &file.old_path {
            g = g.arg(old);
        }
        Ok(parse_diff(&g.run()?))
    }
}

fn parse_hunk_header(line: &str) -> Option<(u32, u32)> {
    // @@ -a,b +c,d @@ ...
    let mut it = line.split(' ');
    it.next()?;
    let old = it.next()?.trim_start_matches('-');
    let new = it.next()?.trim_start_matches('+');
    let start = |s: &str| s.split(',').next()?.parse::<u32>().ok();
    Some((start(old)?, start(new)?))
}

pub fn parse_diff(raw: &str) -> Diff {
    let mut diff = Diff::default();
    let mut longest = 0usize;
    let mut in_hunk = false;
    let (mut old_no, mut new_no) = (0u32, 0u32);
    for line in raw.lines() {
        if line.starts_with("@@") {
            in_hunk = true;
            if let Some((o, n)) = parse_hunk_header(line) {
                old_no = o;
                new_no = n;
            }
            diff.hunks.push(DiffHunk {
                header_index: diff.lines.len(),
            });
            diff.lines.push(DiffLine {
                kind: DiffLineKind::Hunk,
                old_line: None,
                new_line: None,
                text: line.to_string(),
            });
            continue;
        }
        if !in_hunk {
            if line.starts_with("Binary files") || line.starts_with("GIT binary patch") {
                diff.is_binary = true;
            }
            continue;
        }
        if line.starts_with("diff --git") {
            // A second file section (shouldn't happen with a single pathspec).
            in_hunk = false;
            continue;
        }
        if diff.lines.len() >= MAX_LINES {
            diff.truncated = true;
            break;
        }
        let (kind, text) = match line.as_bytes().first() {
            Some(b'+') => (DiffLineKind::Add, &line[1..]),
            Some(b'-') => (DiffLineKind::Delete, &line[1..]),
            Some(b'\\') => (DiffLineKind::NoNewline, line),
            Some(b' ') => (DiffLineKind::Context, &line[1..]),
            None => (DiffLineKind::Context, ""),
            _ => (DiffLineKind::Context, line),
        };
        let (o, n) = match kind {
            DiffLineKind::Add => {
                diff.added += 1;
                new_no += 1;
                (None, Some(new_no - 1))
            }
            DiffLineKind::Delete => {
                diff.deleted += 1;
                old_no += 1;
                (Some(old_no - 1), None)
            }
            DiffLineKind::Context => {
                old_no += 1;
                new_no += 1;
                (Some(old_no - 1), Some(new_no - 1))
            }
            _ => (None, None),
        };
        // Tabs render at inconsistent widths in proportional fallbacks;
        // expand once here instead of at every paint.
        let text = text.replace('\t', "    ");
        if text.len() > longest {
            longest = text.len();
            diff.longest_line = diff.lines.len();
        }
        diff.lines.push(DiffLine {
            kind,
            old_line: o,
            new_line: n,
            text,
        });
    }
    diff
}
