//! One row of the unified diff: `[handle][check · old · new][marker][text]`.
//! A plain fn, not a `RenderOnce`: built for every visible row every frame
//! (hot-row exception, see docs/CONVENTIONS.md).

use gd_git::{Diff, DiffLineKind};
use gd_ui::Theme;
use gpui::{AnyElement, Div, Hsla, IntoElement, ParentElement, Pixels, Styled, div, prelude::*, px};

use super::expander::{Expander, header_hunk, trailing_row};
use super::gutter_style::{HANDLE_W, number_cell_w};
use super::highlight::{DiffHighlight, line_text};
use super::number_cell::number_cell;
use super::row_select::{BlockRow, RowSelect};
use super::text_flow::{TextFlow, marker_cell, row_box, text_cell};
use crate::repo::RepoModel;

/// What every unified row needs besides its index.
pub struct UnifiedRows<'a> {
    pub diff: &'a Diff,
    pub hl: Option<&'a DiffHighlight>,
    pub sel: Option<(&'a RowSelect, &'a RepoModel)>,
    pub exp: Option<&'a Expander>,
    pub num_w: Pixels,
    pub flow: TextFlow,
    pub theme: &'a Theme,
}

/// Row background, text color and `+`/`-` marker of a line kind.
struct LineLook {
    bg: Hsla,
    fg: Hsla,
    marker: &'static str,
    marker_color: Hsla,
}

fn line_look(kind: DiffLineKind, theme: &Theme) -> LineLook {
    let (bg, fg, marker) = match kind {
        DiffLineKind::Add => (theme.diff_add_bg, theme.text, "+"),
        DiffLineKind::Delete => (theme.diff_del_bg, theme.text, "-"),
        DiffLineKind::Hunk => (theme.diff_hunk_bg, theme.text_faint, ""),
        DiffLineKind::NoNewline => (gpui::transparent_black(), theme.text_faint, ""),
        DiffLineKind::Context => (gpui::transparent_black(), theme.text_muted, " "),
    };
    let marker_color = match kind {
        DiffLineKind::Add => theme.diff_add,
        DiffLineKind::Delete => theme.diff_del,
        _ => theme.text_faint,
    };
    LineLook { bg, fg, marker, marker_color }
}

impl UnifiedRows<'_> {
    fn gutter_w(&self) -> Pixels {
        number_cell_w(self.num_w, 2.0, self.sel.is_some()) + if self.sel.is_some() { px(HANDLE_W) } else { px(0.0) }
    }

    pub fn row(&self, ix: usize) -> AnyElement {
        if ix == self.diff.lines.len() {
            return trailing_row(self.exp, self.diff, self.gutter_w(), self.flow, self.theme);
        }
        let line = &self.diff.lines[ix];
        let look = line_look(line.kind, self.theme);
        let hunk = line.kind == DiffLineKind::Hunk;
        let row = row_box(self.flow)
            .bg(look.bg)
            .when(hunk && ix > 0, |d| d.border_t_1().border_color(self.theme.border))
            .when(hunk, |d| d.text_color(self.theme.text_faint).italic());
        let text = self.text(ix, look.fg);
        match header_hunk(self.diff, ix) {
            // Hunk headers carry the expand buttons in their gutter.
            Some(h) => self.header_row(row, h, text, look.marker_color),
            None => self.line_row(row, ix, text, &look),
        }
    }

    fn text(&self, ix: usize, fg: Hsla) -> AnyElement {
        let theme = self.theme;
        let line = &self.diff.lines[ix];
        let font = gpui::font(theme.font_mono.clone());
        let emph_bg = if line.kind == DiffLineKind::Add { theme.diff_add_emph } else { theme.diff_del_emph };
        let spans = self.hl.map(|h| h.row(ix)).unwrap_or(&[]);
        line_text(&line.text, spans, &line.emphasis, fg, emph_bg, &font, &theme.syntax)
    }

    fn header_row(&self, row: Div, h: usize, text: AnyElement, marker_color: Hsla) -> AnyElement {
        row.child(Expander::gutter(self.exp, self.diff, h, self.gutter_w()).border_r_1().border_color(self.theme.border))
            .child(marker_cell("", marker_color))
            .child(text_cell(text, self.flow))
            .into_any_element()
    }

    fn line_row(&self, row: Div, ix: usize, text: AnyElement, look: &LineLook) -> AnyElement {
        let (diff, theme) = (self.diff, self.theme);
        let line = &diff.lines[ix];
        row
            // One selection cell per line, like Desktop's unified view.
            .when_some(self.sel, |d, (s, r)| {
                let block = RepoModel::change_block(diff, ix);
                let first = block.as_ref().is_some_and(|b| b.start == ix);
                d.child(s.handle_cell(block, if first { BlockRow::First } else { BlockRow::Rest }, ix, r, theme))
            })
            .child(div().flex().flex_none().border_r_1().border_color(theme.border).child(number_cell(
                self.sel,
                diff,
                Some(ix),
                &[line.old_line, line.new_line],
                ("line-no", ix),
                self.num_w,
                theme,
            )))
            .child(marker_cell(look.marker, look.marker_color))
            .child(text_cell(text, self.flow))
            .into_any_element()
    }
}
