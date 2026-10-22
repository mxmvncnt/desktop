//! One row of the split diff: `[old text | old no · handle · new no | new text]`,
//! like Desktop. Plain fns, not `RenderOnce`: built for every visible row every
//! frame (hot-row exception, see docs/CONVENTIONS.md).

use gd_git::DiffLineKind;
use gd_ui::{Theme, theme::ink};
use gpui::{AnyElement, Div, IntoElement, ParentElement, Pixels, Styled, div, prelude::*, px};

use super::expander::{Expander, header_hunk, trailing_row};
use super::highlight::{DiffHighlight, line_text};
use super::number_cell::number_cell;
use super::row_select::{BlockRow, RowSelect};
use super::split_diff::{NONE, SplitDiff, SplitRow};
use super::text_flow::{TextFlow, marker_cell, row_box, text_cell};
use crate::repo::RepoModel;

/// What every split row needs besides its index.
pub struct SplitRows<'a> {
    pub split: &'a SplitDiff,
    pub hl: Option<&'a DiffHighlight>,
    pub sel: Option<(&'a RowSelect, &'a RepoModel)>,
    pub exp: Option<&'a Expander>,
    pub num_w: Pixels,
    /// Width of the expand-button gutter of header/trailing rows.
    pub exp_w: Pixels,
    pub flow: TextFlow,
    pub theme: &'a Theme,
}

impl SplitRows<'_> {
    pub fn row(&self, ix: usize) -> AnyElement {
        let diff = &self.split.diff;
        if ix == self.split.rows.len() {
            return trailing_row(self.exp, diff, self.exp_w, self.flow, self.theme);
        }
        let row = self.split.rows[ix];
        if row.left == row.right && diff.lines[row.left as usize].kind == DiffLineKind::Hunk {
            return self.header_row(ix, row);
        }
        row_box(self.flow)
            .child(self.side(row.left))
            .child(self.gutter(ix, row))
            .child(self.side(row.right))
            .into_any_element()
    }

    /// Hunk headers span both columns and carry the expand buttons. They put
    /// them where the middle gutter is narrowest to reach: at the left edge,
    /// one gutter column wide.
    fn header_row(&self, ix: usize, row: SplitRow) -> AnyElement {
        let (diff, theme) = (&self.split.diff, self.theme);
        let line = &diff.lines[row.left as usize];
        let h = header_hunk(diff, row.left as usize).unwrap_or(0);
        row_box(self.flow)
            .bg(theme.diff_hunk_bg)
            .italic()
            .text_color(theme.text_faint)
            .when(ix > 0, |d| d.border_t_1().border_color(theme.border))
            .child(Expander::gutter(self.exp, diff, h, self.exp_w).border_r_1().border_color(theme.border))
            .child(div().pl(px(8.0)).child(line.text.clone()))
            .into_any_element()
    }

    /// Middle gutter. Only the number of the side that owns a change (deletes
    /// left, adds right) is clickable.
    fn gutter(&self, ix: usize, row: SplitRow) -> Div {
        let (diff, theme, sel) = (&self.split.diff, self.theme, self.sel);
        let some = |i: u32| (i != NONE).then_some(i as usize);
        let (left, right) = (some(row.left), some(row.right));
        let old_no = left.and_then(|i| diff.lines[i].old_line);
        let new_no = right.and_then(|i| diff.lines[i].new_line);
        div()
            .flex()
            .flex_none()
            .border_l_1()
            .border_r_1()
            .border_color(theme.border)
            .child(number_cell(sel, diff, left, &[old_no], ("old-no", ix), self.num_w, theme))
            .when_some(sel, |d, (s, r)| {
                // A paired row belongs to the block of its changed side(s).
                let block = [left, right].into_iter().flatten().find_map(|i| RepoModel::change_block(diff, i));
                let first = block.as_ref().is_some_and(|b| left == Some(b.start) || right == Some(b.start));
                d.child(s.handle_cell(block, if first { BlockRow::First } else { BlockRow::Rest }, ix, r, theme))
            })
            .child(number_cell(sel, diff, right, &[new_no], ("new-no", ix), self.num_w, theme))
    }

    /// One text side: change marker + line text.
    fn side(&self, ix: u32) -> Div {
        let theme = self.theme;
        let cell = div().flex().flex_1().min_w_0().overflow_hidden();
        if ix == NONE {
            return cell.bg(ink(0.025));
        }
        let line = &self.split.diff.lines[ix as usize];
        let (bg, fg, marker, marker_color) = match line.kind {
            DiffLineKind::Add => (theme.diff_add_bg, theme.text, "+", theme.diff_add),
            DiffLineKind::Delete => (theme.diff_del_bg, theme.text, "-", theme.diff_del),
            DiffLineKind::NoNewline => (gpui::transparent_black(), theme.text_faint, "", theme.text_faint),
            _ => (gpui::transparent_black(), theme.text_muted, " ", theme.text_faint),
        };
        let font = gpui::font(theme.font_mono.clone());
        let emph_bg = if line.kind == DiffLineKind::Add { theme.diff_add_emph } else { theme.diff_del_emph };
        let spans = self.hl.map(|h| h.row(ix as usize)).unwrap_or(&[]);
        let text: AnyElement = line_text(&line.text, spans, &line.emphasis, fg, emph_bg, &font, &theme.syntax);
        cell.bg(bg).child(marker_cell(marker, marker_color)).child(text_cell(text, self.flow))
    }
}
