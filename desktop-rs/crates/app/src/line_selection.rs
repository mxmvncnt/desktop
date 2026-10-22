//! Partial (line / hunk) selection for the next commit, Desktop's diff
//! gutter checkboxes.
//!
//! A file is in one of three states:
//! - fully included (default): not in `excluded`, no entry in `partial`;
//! - excluded: its path is in `RepoModel::excluded`;
//! - partial: an entry in `RepoModel::partial` lists the deselected rows of
//!   the diff the selection was made on. That diff (with its full texts) is
//!   kept, so the staged blob is built from exactly what the user saw.
//!
//! Rows are indices into `Diff::lines`; only adds and deletes are selectable.
//! Updates normalize back to "fully included" or "excluded" when every row is
//! (de)selected, so `partial` only holds genuinely mixed files.

use std::collections::HashSet;
use std::sync::Arc;

use gd_git::{Diff, FileStatus, PartialFile};
use gd_ui::CheckState;
use gpui::Context;

use crate::repo::{RepoModel, Tab};

pub struct LineSelection {
    pub diff: Arc<Diff>,
    /// Deselected selectable rows. Never empty, never all rows.
    pub excluded: HashSet<usize>,
}

impl RepoModel {
    /// The changes-list checkbox state of `path`.
    pub fn file_check_state(&self, path: &str) -> CheckState {
        if self.excluded.contains(path) {
            CheckState::Unchecked
        } else if self.partial.contains_key(path) {
            CheckState::Mixed
        } else {
            CheckState::Checked
        }
    }

    /// Lines of the displayed working diff can be clicked: plain text diff of
    /// the Changes tab, whitespace shown, no merge in progress.
    pub fn can_select_lines(&self) -> bool {
        let Some(diff) = &self.change_diff else { return false };
        let conflicted = self
            .selected_change
            .and_then(|ix| self.changes().get(ix))
            .is_none_or(|c| c.status == FileStatus::Conflicted);
        self.tab == Tab::Changes
            && !self.stashes.viewing
            && !conflicted
            && !self.diff_options.ignore_whitespace
            && diff.source.is_some()
            && !diff.truncated
            && self.merge.in_progress.as_ref().is_none_or(|p| !p.commit_finishes())
    }

    fn selected_path(&self) -> Option<String> {
        self.selected_change.and_then(|ix| self.changes().get(ix)).map(|c| c.path.clone())
    }

    /// Whether row `ix` of the displayed working diff goes into the next commit.
    pub fn row_included(&self, ix: usize) -> bool {
        let Some(path) = self.selected_change.and_then(|ix| self.changes().get(ix)).map(|c| &c.path) else {
            return false;
        };
        if self.excluded.contains(path) {
            return false;
        }
        self.partial.get(path).is_none_or(|s| !s.excluded.contains(&ix))
    }

    /// Include or exclude `rows` of the displayed diff. Returns whether
    /// anything changed (callers dragging over the gutter call this per move).
    pub fn set_rows_included(
        &mut self,
        rows: impl IntoIterator<Item = usize>,
        include: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.can_select_lines() {
            return false;
        }
        let (Some(path), Some(diff)) = (self.selected_path(), self.change_diff.clone()) else { return false };
        let all: Vec<usize> = (0..diff.lines.len()).filter(|&i| gd_git::selectable(&diff, i)).collect();
        let mut excluded: HashSet<usize> = if self.excluded.contains(&path) {
            all.iter().copied().collect()
        } else {
            self.partial.get(&path).map(|s| s.excluded.clone()).unwrap_or_default()
        };
        let mut changed = false;
        for ix in rows.into_iter().filter(|&i| gd_git::selectable(&diff, i)) {
            changed |= if include { excluded.remove(&ix) } else { excluded.insert(ix) };
        }
        if !changed {
            return false;
        }
        self.partial.remove(&path);
        self.excluded.remove(&path);
        if excluded.len() == all.len() {
            self.excluded.insert(path);
        } else if !excluded.is_empty() {
            self.partial.insert(path, LineSelection { diff, excluded });
        }
        cx.notify();
        true
    }

    pub fn toggle_row(&mut self, ix: usize, cx: &mut Context<Self>) -> bool {
        let include = !self.row_included(ix);
        self.set_rows_included([ix], include, cx);
        include
    }

    /// The change block containing row `ix`: the run of adds/deletes
    /// around it, bounded by context lines or hunk headers. This is what
    /// the hunk handle (de)selects. `None` for a non-change row.
    pub fn change_block(diff: &Diff, ix: usize) -> Option<std::ops::Range<usize>> {
        use gd_git::DiffLineKind::{Add, Delete, NoNewline};
        let in_block = |i: usize| matches!(diff.lines[i].kind, Add | Delete | NoNewline);
        if !gd_git::selectable(diff, ix) {
            return None;
        }
        let mut start = ix;
        while start > 0 && in_block(start - 1) {
            start -= 1;
        }
        let mut end = ix + 1;
        while end < diff.lines.len() && in_block(end) {
            end += 1;
        }
        Some(start..end)
    }

    /// Selection state of a change block (all / none / some lines included).
    pub fn block_state(&self, rows: std::ops::Range<usize>) -> CheckState {
        let Some(diff) = &self.change_diff else { return CheckState::Unchecked };
        let (mut on, mut off) = (false, false);
        for ix in rows.filter(|&i| gd_git::selectable(diff, i)) {
            if self.row_included(ix) { on = true } else { off = true }
        }
        match (on, off) {
            (true, false) => CheckState::Checked,
            (false, _) => CheckState::Unchecked,
            _ => CheckState::Mixed,
        }
    }

    /// Select the whole block, or deselect it if it was fully selected.
    pub fn toggle_block(&mut self, rows: std::ops::Range<usize>, cx: &mut Context<Self>) {
        let include = self.block_state(rows.clone()) != CheckState::Checked;
        self.set_rows_included(rows, include, cx);
    }

    /// A freshly loaded diff of `path`: keep its partial selection only if
    /// the diff is unchanged, since row indices refer to the old one.
    pub(crate) fn reconcile_selection(&mut self, path: &str, diff: &Arc<Diff>) {
        if self.diff_options.ignore_whitespace {
            // A `-w` diff has different rows; keep the selection for later.
            return;
        }
        if let Some(sel) = self.partial.get_mut(path) {
            if sel.diff.lines == diff.lines {
                sel.diff = diff.clone();
            } else {
                self.partial.remove(path);
            }
        }
    }

    /// Files with only some lines selected, with their staged content.
    pub fn partial_files(&self) -> Vec<PartialFile> {
        self.changes()
            .iter()
            .filter(|c| !self.excluded.contains(&c.path))
            .filter_map(|c| {
                let sel = self.partial.get(&c.path)?;
                let content = gd_git::staged_text(&sel.diff, |ix| !sel.excluded.contains(&ix))?;
                Some(PartialFile {
                    path: c.path.clone(),
                    old_path: c.old_path.clone(),
                    content,
                    source: sel.diff.source.clone()?,
                })
            })
            .collect()
    }

    /// Anything (whole files or some lines) is selected for commit.
    pub fn has_included(&self) -> bool {
        self.changes().iter().any(|c| !self.excluded.contains(&c.path))
    }
}
