//! `Workspace::render_diff`: the entry point every pane (changes, history,
//! stashes) calls to show a diff.

use std::sync::Arc;

use gd_git::Diff;
use gd_ui::Theme;
use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, div, point, px};

use super::diff_frame::DiffFrame;
use super::diff_pane::DiffPane;
use super::expander::{ExpandTarget, Expander};
use super::gutter_style::{HANDLE_W, number_cell_w};
use super::highlight::DiffHighlight;
use super::row_select::RowSelect;
use super::split_list::SplitList;
use super::whitespace_note::WhitespaceNote;
use super::state::DiffSelectUi;
use super::text_flow::{MARKER_W, TextFlow};
use super::text_select::TextSelect;
use super::unified_list::UnifiedList;
use crate::shared::overscroll::ScrollProbe;
use crate::workspace::Workspace;

impl Workspace {
    pub fn render_diff(
        &mut self,
        diff: Option<Arc<Diff>>,
        highlight: Option<Arc<DiffHighlight>>,
        key: SharedString,
        pane: DiffPane,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::of(cx).clone();
        let new_key = self.reset_for_key(&key);
        let Some(diff) = diff else {
            return div().flex_1().into_any_element();
        };
        if let Some(message) = self.placeholder_message(&diff) {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.text_faint)
                .child(message)
                .into_any_element();
        }
        let note = WhitespaceNote::new(self.settings.diff_ignore_whitespace);
        let (select, exp) = self.pane_parts(pane, &diff, cx);
        let wrap = self.settings.diff_wrap;
        let flow = if wrap { TextFlow::Wrap } else { TextFlow::Scroll(self.diff_view.hscroll) };
        let split = self.settings.diff_split.then(|| self.split_for(&diff));
        let trailing = usize::from(exp.is_some() && Expander::has_trailing(&diff));
        let n = split.as_ref().map_or(diff.lines.len(), |s| s.rows.len()) + trailing;
        let wrap_list = wrap.then(|| self.diff_view.wrap_list_for(&diff, split.is_some(), n, new_key));
        let truncated = diff.truncated;
        let mono = theme.font_mono.clone();
        let scroll = self.diff_scroll.clone();
        let probe = wrap_list.clone().map_or_else(|| ScrollProbe::from(&scroll), ScrollProbe::from);
        let tsel = TextSelect { ws: cx.entity(), sel: self.diff_view.text_sel, color: theme.selection };
        let list = match split {
            Some(split) => {
                self.set_split_extent(&diff, split.gutter_digits, select.is_some());
                SplitList { split, hl: highlight, select, exp, trailing: trailing == 1, scroll, wrap: wrap_list, flow, tsel, mono }
                    .into_any_element()
            }
            None => {
                let num_w = self.set_unified_extent(&diff, select.is_some());
                UnifiedList { diff, hl: highlight, select, exp, rows: n, num_w, scroll, wrap: wrap_list, flow, tsel, mono }
                    .into_any_element()
            }
        };
        DiffFrame { key, list, probe, truncated, flow, note: note.map(IntoElement::into_any_element), ws: cx.entity() }.into_any_element()
    }

    /// New file/commit selected: start at the top-left. A reload of the same
    /// file (same key) keeps the scroll position. Returns whether the key changed.
    fn reset_for_key(&mut self, key: &SharedString) -> bool {
        let new_key = self.diff_key.as_ref() != Some(key);
        if new_key {
            self.diff_key = Some(key.clone());
            self.diff_scroll.0.borrow().base_handle.set_offset(point(px(0.0), px(0.0)));
            self.diff_view.hscroll = px(0.0);
            self.diff_select = DiffSelectUi::default();
            self.diff_view.text_sel = None;
        }
        new_key
    }

    /// Text shown instead of the diff (binary file, nothing to show).
    fn placeholder_message(&self, diff: &Diff) -> Option<&'static str> {
        if diff.is_binary {
            Some("Binary file not shown")
        } else if !diff.lines.is_empty() {
            None
        } else if self.settings.diff_ignore_whitespace {
            Some("No content changes (only whitespace differs, or a mode change)")
        } else {
            Some("No content changes (mode change or empty file)")
        }
    }

    /// Line selection and hunk expansion available in `pane`.
    fn pane_parts(&self, pane: DiffPane, diff: &Diff, cx: &Context<Self>) -> (Option<RowSelect>, Option<Expander>) {
        let (select, exp) = match pane {
            DiffPane::Changes(repo) => {
                (Some(repo.clone()), Some(Expander { repo, target: ExpandTarget::Changes }))
            }
            DiffPane::History(repo) => (None, Some(Expander { repo, target: ExpandTarget::Commit })),
            DiffPane::ReadOnly => (None, None),
        };
        let select = select.filter(|r| r.read(cx).can_select_lines()).map(|repo| RowSelect {
            repo,
            ws: cx.entity(),
            hovered_block: self.diff_select.hovered_handle.map(|h| h.block),
        });
        (select, exp.filter(|_| gd_git::expandable(diff)))
    }

    /// Records the horizontal layout of a split diff for `scroll_diff_x`.
    fn set_split_extent(&mut self, diff: &Arc<Diff>, gutter_digits: usize, has_select: bool) {
        let num_w = px(gutter_digits as f32 * 7.5 + 16.0);
        // Middle gutter (two number cells, the handle, two borders) and two markers.
        let fixed = number_cell_w(num_w, 1.0, has_select) * 2.0
            + if has_select { px(HANDLE_W) } else { px(0.0) }
            + px(2.0 + 2.0 * MARKER_W);
        self.diff_view.set_extent(diff, fixed, 2.0);
    }

    /// Records the horizontal layout of a unified diff; returns the number column width.
    fn set_unified_extent(&mut self, diff: &Arc<Diff>, has_select: bool) -> gpui::Pixels {
        let digits = diff
            .lines
            .iter()
            .filter_map(|l| l.new_line.max(l.old_line))
            .max()
            .unwrap_or(1)
            .to_string()
            .len()
            .max(3);
        let num_w = px(digits as f32 * 7.5 + 16.0);
        let fixed = number_cell_w(num_w, 2.0, has_select)
            + if has_select { px(HANDLE_W) } else { px(0.0) }
            + px(1.0 + MARKER_W);
        self.diff_view.set_extent(diff, fixed, 1.0);
        num_w
    }
}
