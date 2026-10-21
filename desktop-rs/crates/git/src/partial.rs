//! Partial (line / hunk) staging.
//!
//! Desktop stages a subset of a file's changed lines by writing a patch and
//! running `git apply --cached`. We already hold both full texts
//! ([`DiffSource`]), so we build the staged blob directly instead:
//! [`staged_text`] replays the diff over the old side, keeping only the
//! selected adds/deletes, and [`Repository::stage_partial`] writes that blob
//! with `hash-object -w` and points the index at it with `update-index`.

use std::sync::Arc;

use crate::engine::DiffSource;
use crate::{Diff, DiffLineKind, GitError, Repository, Result, Side};

/// One file committed with only some of its lines.
#[derive(Debug, Clone)]
pub struct PartialFile {
    pub path: String,
    /// Old path of a rename; removed from the index.
    pub old_path: Option<String>,
    /// The staged file content (from [`staged_text`]).
    pub content: String,
    /// The texts the selection was made against. Staging refuses to run if
    /// the working-tree file no longer matches `source.new`.
    pub source: Arc<DiffSource>,
}

/// Whether row `ix` of a diff can be (de)selected for commit.
pub fn selectable(diff: &Diff, ix: usize) -> bool {
    matches!(diff.lines.get(ix).map(|l| l.kind), Some(DiffLineKind::Add | DiffLineKind::Delete))
}

/// Content of the file with only the rows for which `include(row)` is true
/// applied to the old side: an included delete drops the old line, an
/// excluded one keeps it; an included add inserts the new line. `None` when
/// the diff has no source (binary, non-UTF-8) or is truncated.
pub fn staged_text(diff: &Diff, include: impl Fn(usize) -> bool) -> Option<String> {
    let src = diff.source.as_ref()?;
    if diff.truncated || diff.too_large {
        return None;
    }
    let old: Vec<&str> = src.old.split_inclusive('\n').collect();
    let new: Vec<&str> = src.new.split_inclusive('\n').collect();
    let mut out = String::with_capacity(src.old.len().max(src.new.len()));
    // A kept unterminated last line followed by more content needs its
    // newline back, or the two lines would merge.
    fn push(out: &mut String, s: &str) {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(s);
    }
    // Copy unchanged old lines `next..end` (the gaps between hunks).
    fn copy_old(out: &mut String, old: &[&str], next: &mut usize, end: usize) {
        for l in old.get(*next..end.min(old.len())).unwrap_or(&[]) {
            push(out, l);
        }
        *next = (*next).max(end);
    }
    let mut next_old = 0usize;
    for (ix, line) in diff.lines.iter().enumerate() {
        match line.kind {
            DiffLineKind::Context => copy_old(&mut out, &old, &mut next_old, line.old_line? as usize),
            DiffLineKind::Delete => {
                let o = line.old_line? as usize - 1;
                copy_old(&mut out, &old, &mut next_old, o);
                if !include(ix) {
                    push(&mut out, old.get(o)?);
                }
                next_old = o + 1;
            }
            DiffLineKind::Add => {
                if include(ix) {
                    push(&mut out, new.get(line.new_line? as usize - 1)?);
                }
            }
            DiffLineKind::Hunk | DiffLineKind::NoNewline => {}
        }
    }
    copy_old(&mut out, &old, &mut next_old, old.len());
    Some(out)
}

impl Repository {
    /// Point the index entry of each file at its partially staged content.
    /// Fails (staging nothing further) if a file changed on disk since its
    /// lines were selected.
    pub fn stage_partial(&self, files: &[PartialFile]) -> Result<()> {
        for f in files {
            let current = self.read_side(&Side::Worktree(f.path.clone())).unwrap_or_default();
            if current != f.source.new.as_bytes() {
                return Err(GitError::Invalid(format!(
                    "{} changed on disk after you selected lines; review the selection and try again",
                    f.path
                )));
            }
        }
        for f in files {
            let sha = self
                .git()
                .args(["hash-object", "-w", "--stdin"])
                .arg(format!("--path={}", f.path))
                .stdin(f.content.clone())
                .run()?
                .trim()
                .to_string();
            let mode = self.index_mode(f)?;
            if let Some(old) = &f.old_path {
                self.git().args(["update-index", "--force-remove", "--"]).arg(old).run()?;
            }
            self.git()
                .args(["update-index", "--add", "--cacheinfo"])
                .arg(format!("{mode},{sha},{}", f.path))
                .run()?;
        }
        Ok(())
    }

    /// File mode for the staged entry: the working-tree executable bit on
    /// unix, else the mode in HEAD, else a regular file.
    fn index_mode(&self, f: &PartialFile) -> Result<&'static str> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(self.root().join(&f.path)) {
                return Ok(if meta.permissions().mode() & 0o111 != 0 { "100755" } else { "100644" });
            }
        }
        if self.has_head() {
            let path = f.old_path.as_deref().unwrap_or(&f.path);
            let out = self.git().args(["ls-tree", "HEAD", "--"]).arg(path).run()?;
            if out.starts_with("100755") {
                return Ok("100755");
            }
        }
        Ok("100644")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiffOptions;
    use crate::engine::diff_texts;

    fn stage(old: &str, new: &str, pick: impl Fn(&crate::DiffLine) -> bool) -> String {
        let d = diff_texts(old, new, DiffOptions::default());
        staged_text(&d, |ix| pick(&d.lines[ix])).unwrap()
    }

    #[test]
    fn all_and_none() {
        let (old, new) = ("a\nb\nc\n", "a\nB\nc\nd\n");
        assert_eq!(stage(old, new, |_| true), new);
        assert_eq!(stage(old, new, |_| false), old);
    }

    #[test]
    fn only_adds_or_only_deletes() {
        let (old, new) = ("a\nb\nc\n", "a\nB\nc\nd\n");
        assert_eq!(stage(old, new, |l| l.kind == DiffLineKind::Add), "a\nb\nB\nc\nd\n");
        assert_eq!(stage(old, new, |l| l.kind == DiffLineKind::Delete), "a\nc\n");
    }

    #[test]
    fn one_line_far_apart() {
        let old: String = (1..=30).map(|i| format!("{i}\n")).collect();
        let new = old.replace("2\n", "two\n").replace("28\n", "twenty-eight\n");
        let got = stage(&old, &new, |l| l.text == "twenty-eight" || l.text == "28");
        assert_eq!(got, old.replace("28\n", "twenty-eight\n"));
    }

    #[test]
    fn missing_final_newline() {
        assert_eq!(stage("a", "a\nb\n", |l| l.text == "b"), "a\nb\n");
        assert_eq!(stage("", "x\ny", |l| l.text == "x"), "x\n");
        assert_eq!(stage("", "x\ny", |l| l.text == "y"), "y");
    }

    #[test]
    fn crlf_kept() {
        assert_eq!(stage("a\r\nb\r\n", "a\r\nB\r\n", |l| l.kind == DiffLineKind::Add), "a\r\nb\r\nB\r\n");
    }
}
