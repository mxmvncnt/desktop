//! Split (side-by-side) diff. The paired row model is built once per loaded
//! diff ([`SplitDiff::build`], one linear pass, cached on the Workspace) and
//! rendered through ONE virtualized `uniform_list` whose rows each hold both
//! halves, so both sides always scroll together.
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
//! Long lines are clipped at the column edge (`overflow_hidden`); the split
//! view has no horizontal scrolling (two independently scrolled columns would
//! need a shared extent, and unified view still scrolls horizontally).

use std::sync::Arc;
use std::time::Instant;

use gd_git::{Diff, DiffLineKind};
use gd_ui::{Theme, theme::DIFF_ROW_HEIGHT, ink};
use gpui::{
    AnyElement, App, Div, IntoElement, ParentElement, StyledText, Styled, div, prelude::*, px,
    uniform_list,
};

use crate::diff_highlight::{DiffHighlight, runs_for};

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

/// One half (cell) of a split row.
fn half(
    diff: &Diff,
    hl: Option<&DiffHighlight>,
    ix: u32,
    left: bool,
    gutter_w: f32,
    theme: &Theme,
) -> Div {
    let cell = div().flex().flex_none().w_1_2().h_full().overflow_hidden();
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
    let no = if left { line.old_line } else { line.new_line };
    let text: AnyElement = match hl.map(|h| h.row(ix as usize)) {
        Some(spans) if !spans.is_empty() => {
            let font = gpui::font(theme.font_mono.clone());
            let runs = runs_for(&line.text, spans, fg, &font, &theme.syntax);
            StyledText::new(line.text.clone()).with_runs(runs).into_any_element()
        }
        _ => div().text_color(fg).child(line.text.clone()).into_any_element(),
    };
    cell.bg(bg)
        .child(
            div()
                .flex_none()
                .w(px(gutter_w))
                .pr(px(8.0))
                .flex()
                .justify_end()
                .text_color(theme.diff_gutter)
                .child(no.map(|n| n.to_string()).unwrap_or_default()),
        )
        .child(
            div()
                .flex_none()
                .w(px(16.0))
                .flex()
                .justify_center()
                .text_color(marker_color)
                .child(marker),
        )
        .child(div().flex_none().child(text))
}

/// The virtualized list for a split diff.
pub fn split_list(
    split: Arc<SplitDiff>,
    hl: Option<Arc<DiffHighlight>>,
    scroll: &gpui::UniformListScrollHandle,
    mono: gpui::SharedString,
) -> AnyElement {
    let n = split.rows.len();
    let gutter_w = split.gutter_digits as f32 * 7.5 + 12.0;
    uniform_list("diff-split", n, move |range, _w, cx: &mut App| {
        let theme = Theme::of(cx).clone();
        let diff = &split.diff;
        range
            .map(|ix| {
                let row = split.rows[ix];
                let base = div().flex().h(px(DIFF_ROW_HEIGHT)).w_full().whitespace_nowrap();
                let line = &diff.lines[row.left as usize];
                if row.left == row.right && line.kind == DiffLineKind::Hunk {
                    return base
                        .bg(theme.diff_hunk_bg)
                        .pl(px(gutter_w + 16.0))
                        .italic()
                        .text_color(theme.text_faint)
                        .when(ix > 0, |d| d.border_t_1().border_color(theme.border))
                        .child(line.text.clone())
                        .into_any_element();
                }
                let hl = hl.as_deref();
                base.child(half(diff, hl, row.left, true, gutter_w, &theme))
                    .child(
                        half(diff, hl, row.right, false, gutter_w, &theme)
                            .border_l_1()
                            .border_color(theme.border),
                    )
                    .into_any_element()
            })
            .collect()
    })
    .track_scroll(scroll)
    .size_full()
    .font_family(mono)
    .text_size(px(12.0))
    .line_height(px(DIFF_ROW_HEIGHT))
    .into_any_element()
}
