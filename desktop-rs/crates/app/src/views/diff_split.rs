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
//! `DiffView::hscroll` (see `TextFlow` in `diff.rs`), so both sides scroll
//! together too while the middle gutter stays put. With wrapping on, each side
//! wraps within its column and the row takes the taller side's height.

use std::sync::Arc;
use std::time::Instant;

use gd_git::{Diff, DiffLineKind};
use gd_ui::{Theme, theme::ink};
use gpui::{AnyElement, Div, IntoElement, ListState, ParentElement, Styled, div, prelude::*, px};

use crate::diff_highlight::{DiffHighlight, line_text};
use crate::repo::RepoModel;
use crate::views::diff::{RowSelect, TextFlow, marker_cell, number_cell, row_box, rows_list, text_cell};

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

/// One text side of a split row: change marker + line text. The line
/// numbers live in the shared middle gutter ([`split_list`]).
fn side(diff: &Diff, hl: Option<&DiffHighlight>, ix: u32, flow: TextFlow, theme: &Theme) -> Div {
    let cell = div().flex().flex_1().min_w_0().overflow_hidden();
    if ix == NONE {
        return cell.bg(ink(0.025));
    }
    let line = &diff.lines[ix as usize];
    let (bg, fg, marker, marker_color) = match line.kind {
        DiffLineKind::Add => (theme.diff_add_bg, theme.text, "+", theme.diff_add),
        DiffLineKind::Delete => (theme.diff_del_bg, theme.text, "-", theme.diff_del),
        DiffLineKind::NoNewline => (gpui::transparent_black(), theme.text_faint, "", theme.text_faint),
        _ => (gpui::transparent_black(), theme.text_muted, " ", theme.text_faint),
    };
    let font = gpui::font(theme.font_mono.clone());
    let emph_bg = if line.kind == DiffLineKind::Add { theme.diff_add_emph } else { theme.diff_del_emph };
    let spans = hl.map(|h| h.row(ix as usize)).unwrap_or(&[]);
    let text: AnyElement = line_text(&line.text, spans, &line.emphasis, fg, emph_bg, &font, &theme.syntax);
    cell.bg(bg).child(marker_cell(marker, marker_color)).child(text_cell(text, flow))
}

/// The virtualized list for a split diff: `[old text | old no · hunk
/// handle · new no | new text]`, like Desktop. Only the number of the side
/// that owns a change (deletes left, adds right) is clickable.
pub fn split_list(
    split: Arc<SplitDiff>,
    hl: Option<Arc<DiffHighlight>>,
    select: Option<RowSelect>,
    scroll: &gpui::UniformListScrollHandle,
    wrap: Option<ListState>,
    flow: TextFlow,
    mono: gpui::SharedString,
) -> AnyElement {
    let n = split.rows.len();
    let num_w = px(split.gutter_digits as f32 * 7.5 + 16.0);
    rows_list("diff-split", n, wrap, scroll, mono, move |ix, theme, cx| {
        let diff = &split.diff;
        let r = select.as_ref().map(|s| s.repo.read(cx));
        let sel = select.as_ref().zip(r);
        let row = split.rows[ix];
        let base = row_box(flow);
        if row.left == row.right && diff.lines[row.left as usize].kind == DiffLineKind::Hunk {
            let line = &diff.lines[row.left as usize];
            return base
                .bg(theme.diff_hunk_bg)
                .pl(px(20.0))
                .italic()
                .text_color(theme.text_faint)
                .when(ix > 0, |d| d.border_t_1().border_color(theme.border))
                .child(line.text.clone())
                .into_any_element();
        }
        let hl = hl.as_deref();
        let some = |i: u32| (i != NONE).then_some(i as usize);
        let (left, right) = (some(row.left), some(row.right));
        let old_no = left.and_then(|i| diff.lines[i].old_line);
        let new_no = right.and_then(|i| diff.lines[i].new_line);
        let gutter = div()
            .flex()
            .flex_none()
            .border_l_1()
            .border_r_1()
            .border_color(theme.border)
            .child(number_cell(sel, diff, left, &[old_no], ("old-no", ix), num_w, theme))
            .when_some(sel, |d, (s, r)| {
                // A paired row belongs to the block of its changed side(s).
                let block = [left, right]
                    .into_iter()
                    .flatten()
                    .find_map(|i| RepoModel::change_block(diff, i));
                let first = block.as_ref().is_some_and(|b| left == Some(b.start) || right == Some(b.start));
                d.child(s.handle_cell(block, first, ix, r, theme))
            })
            .child(number_cell(sel, diff, right, &[new_no], ("new-no", ix), num_w, theme));
        base.child(side(diff, hl, row.left, flow, theme))
            .child(gutter)
            .child(side(diff, hl, row.right, flow, theme))
            .into_any_element()
    })
}
