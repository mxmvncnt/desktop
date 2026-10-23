//! Hunk expansion buttons in the gutter of hunk header rows (Desktop's expand
//! arrows) and the trailing "expand down" row.

use gd_git::{Diff, Expansion};
use gd_ui::{Icon, Theme};
use gpui::{AnyElement, Div, Entity, IntoElement, ParentElement, Pixels, Styled, div};

use super::expand_button::ExpandButton;
use super::gutter_style::handle_bg;
use super::text_flow::{TextFlow, row_box};
use crate::repo::RepoModel;

/// Which diff the buttons expand.
#[derive(Clone, Copy, PartialEq)]
pub enum ExpandTarget {
    /// `RepoModel::change_diff`.
    Changes,
    /// `RepoModel::commit_diff`.
    Commit,
}

#[derive(Clone)]
pub struct Expander {
    pub repo: Entity<RepoModel>,
    pub target: ExpandTarget,
}

impl Expander {
    /// Whether a trailing "expand down" row follows the last hunk.
    pub fn has_trailing(diff: &Diff) -> bool {
        diff.hunks.len().checked_sub(1).is_some_and(|last| gd_git::gap_below(diff, last) > 0)
    }

    /// Buttons for hunk header `h`, or for the trailing row (`h == hunks.len()`):
    /// first hunk: up; between hunks: "expand all" when the gap is at most one
    /// step, else down (previous hunk) + up; trailing row: down.
    fn buttons(&self, diff: &Diff, h: usize) -> Vec<ExpandButton> {
        use Expansion::{All, Down, Up};
        let n = diff.hunks.len();
        let mut out = Vec::new();
        let mut add = |k: usize, ic: Icon, tip: &'static str, e| {
            out.push(ExpandButton::new(h * 2 + k, ic, tip, e, self.repo.clone(), self.target))
        };
        if h == n {
            if Self::has_trailing(diff) {
                add(0, Icon::ArrowDown, "Expand Down", Down(n - 1));
            }
        } else if gd_git::gap_above(diff, h) == 0 {
        } else if h == 0 {
            add(0, Icon::ArrowUp, "Expand Up", Up(0));
        } else if gd_git::gap_above(diff, h) <= gd_git::EXPANSION_STEP {
            add(0, Icon::SortVertical, "Expand All", All(h));
        } else {
            add(0, Icon::ArrowDown, "Expand Down", Down(h - 1));
            add(1, Icon::ArrowUp, "Expand Up", Up(h));
        }
        out
    }

    /// The gutter of a hunk header (or trailing) row, `width` wide.
    pub fn gutter(exp: Option<&Self>, diff: &Diff, h: usize, width: Pixels) -> Div {
        div()
            .flex()
            .flex_none()
            .w(width)
            .bg(handle_bg())
            .children(exp.map(|e| e.buttons(diff, h)).unwrap_or_default())
    }
}

/// The trailing row after the last hunk: an "expand down" gutter.
pub fn trailing_row(exp: Option<&Expander>, diff: &Diff, width: Pixels, flow: TextFlow, theme: &Theme) -> AnyElement {
    row_box(flow)
        .bg(theme.diff_hunk_bg)
        .border_t_1()
        .border_color(theme.border)
        .child(Expander::gutter(exp, diff, diff.hunks.len(), width).border_r_1().border_color(theme.border))
        .into_any_element()
}

/// Hunk index of header row `ix`.
pub fn header_hunk(diff: &Diff, ix: usize) -> Option<usize> {
    diff.hunks.binary_search_by_key(&ix, |h| h.header_index).ok()
}
