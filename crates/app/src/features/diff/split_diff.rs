//! Split (side-by-side) diff. The paired row model is built once per loaded
//! diff ([`SplitDiff::build`], one linear pass, cached on the Workspace) and
//! rendered through ONE virtualized list whose rows each hold both halves, so
//! both sides always scroll together vertically.
//!
//! Row model: a [`SplitRow`] is a pair of indices into `Diff::lines`.
//! - context: `(i, i)` (shown on both sides);
//! - hunk header: `(i, i)` with `lines[i].kind == Hunk`, spanning both columns;
//! - each run of deletes followed by adds is paired row by row; the shorter
//!   side is padded with [`NONE`] (an empty filler cell);
//! - "\ No newline" markers join the run of the side they follow.
//!
//! Syntax spans come straight from `DiffHighlight::row(ix)` of the underlying
//! line (deletes carry old-side spans, adds/context new-side spans).
//!
//! Horizontally, each column clips its text and shifts it by the one shared
//! `DiffView::hscroll` (see `TextFlow` in `text_flow.rs`), so both sides scroll
//! together too while the middle gutter stays put. With wrapping on, each side
//! wraps within its column and the row takes the taller side's height.

use std::sync::Arc;
use std::time::Instant;

use gd_git::{Diff, DiffLineKind};

/// "No line on this side" (filler cell).
pub const NONE: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitRow {
    pub left: u32,
    pub right: u32,
}

pub struct SplitDiff {
    /// Keeps the source alive so the cache can be validated with `Arc::ptr_eq`.
    pub diff: Arc<Diff>,
    pub rows: Vec<SplitRow>,
    /// Width (in digits) the line-number gutter needs.
    pub gutter_digits: usize,
}

impl SplitDiff {
    pub fn build(diff: &Arc<Diff>) -> Self {
        let started = Instant::now();
        let mut rows = Vec::with_capacity(diff.lines.len());
        let (mut dels, mut adds): (Vec<u32>, Vec<u32>) = (Vec::new(), Vec::new());
        let mut last = DiffLineKind::Context;
        let mut max_no = 1u32;
        fn flush(rows: &mut Vec<SplitRow>, dels: &mut Vec<u32>, adds: &mut Vec<u32>) {
            for i in 0..dels.len().max(adds.len()) {
                rows.push(SplitRow {
                    left: dels.get(i).copied().unwrap_or(NONE),
                    right: adds.get(i).copied().unwrap_or(NONE),
                });
            }
            dels.clear();
            adds.clear();
        }
        for (ix, line) in diff.lines.iter().enumerate() {
            let ix = ix as u32;
            max_no = max_no.max(line.old_line.unwrap_or(0)).max(line.new_line.unwrap_or(0));
            match line.kind {
                DiffLineKind::Delete => {
                    if !adds.is_empty() {
                        flush(&mut rows, &mut dels, &mut adds);
                    }
                    dels.push(ix);
                }
                DiffLineKind::Add => adds.push(ix),
                DiffLineKind::NoNewline => match last {
                    DiffLineKind::Delete => dels.push(ix),
                    DiffLineKind::Add => adds.push(ix),
                    _ => {
                        flush(&mut rows, &mut dels, &mut adds);
                        rows.push(SplitRow { left: ix, right: ix });
                    }
                },
                DiffLineKind::Context | DiffLineKind::Hunk => {
                    flush(&mut rows, &mut dels, &mut adds);
                    rows.push(SplitRow { left: ix, right: ix });
                }
            }
            last = line.kind;
        }
        flush(&mut rows, &mut dels, &mut adds);
        log::info!(
            "split: {} paired rows from {} lines in {:?}",
            rows.len(),
            diff.lines.len(),
            started.elapsed()
        );
        Self { diff: diff.clone(), rows, gutter_digits: max_no.to_string().len().max(3) }
    }
}
