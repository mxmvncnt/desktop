//! Virtualized unified diff. Each row is one `DiffLine`; only visible rows
//! are built per frame, so a 50k-line diff costs the same as a 50-line one.

use std::sync::Arc;

use crate::diff_highlight::{DiffHighlight, line_text};
use crate::repo::RepoModel;
use gd_git::{Diff, DiffLineKind};
use gd_ui::{CheckState, Checkbox, Theme, motion, theme::{DIFF_ROW_HEIGHT, ink}};
use gpui::{
    App, Context, Div, Entity, Hsla, IntoElement, MouseButton, ParentElement, SharedString, Stateful, Styled,
    div, point, prelude::*, px, uniform_list,
};

use crate::workspace::Workspace;

/// Pointer state of the line-selection gutter (see `line_selection.rs`).
#[derive(Default)]
pub struct DiffSelectUi {
    /// Hunk whose handle is hovered: its rows get a wash.
    pub hovered_hunk: Option<usize>,
    /// Gutter drag: the row it started on and whether it is including.
    pub drag: Option<(usize, bool)>,
}

/// Everything a row needs to draw and handle the selection gutter.
#[derive(Clone)]
pub struct RowSelect {
    pub repo: Entity<RepoModel>,
    pub ws: Entity<Workspace>,
    pub hovered_hunk: Option<usize>,
}

/// Hunk index of row `ix` (rows before the first hunk map to 0).
pub fn hunk_of(diff: &Diff, ix: usize) -> usize {
    diff.hunks.partition_point(|h| h.header_index <= ix).saturating_sub(1)
}

impl RowSelect {
    /// The clickable line-number gutter of selectable row `ix`. Included rows
    /// get a blue outline; adjacent included rows merge into one block. In
    /// split view (`split`) only same-kind rows are adjacent on screen.
    pub fn gutter(
        &self,
        diff: &Diff,
        ix: usize,
        id: &'static str,
        split: bool,
        r: &RepoModel,
        theme: &Theme,
    ) -> Stateful<Div> {
        let el = div().id((id, ix)).flex().flex_none().h_full();
        let hunk = hunk_of(diff, ix);
        let hovered = self.hovered_hunk == Some(hunk);
        if !gd_git::selectable(diff, ix) {
            return el.when(hovered, |d| d.bg(ink(0.04)));
        }
        let kind = diff.lines[ix].kind;
        let inc = |i: usize| {
            gd_git::selectable(diff, i) && (!split || diff.lines[i].kind == kind) && r.row_included(i)
        };
        let included = r.row_included(ix);
        let (repo_down, ws_down) = (self.repo.clone(), self.ws.clone());
        let (repo_move, ws_move) = (self.repo.clone(), self.ws.clone());
        el.cursor_pointer()
            .text_color(if included { theme.text } else { theme.diff_gutter })
            .when(included, |d| {
                d.bg(theme.accent_wash)
                    .border_l_1()
                    .border_r_1()
                    .border_color(theme.accent)
                    .when(ix == 0 || !inc(ix - 1), |d| d.border_t_1())
                    .when(!inc(ix + 1), |d| d.border_b_1())
            })
            .when(!included && hovered, |d| d.bg(ink(0.04)))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                let include = repo_down.update(cx, |r, cx| r.toggle_row(ix, cx));
                ws_down.update(cx, |w, _| w.diff_select.drag = Some((ix, include)));
            })
            .on_mouse_move(move |e, _, cx| {
                if e.pressed_button != Some(MouseButton::Left) {
                    return;
                }
                if let Some((anchor, include)) = ws_move.read(cx).diff_select.drag {
                    repo_move.update(cx, |r, cx| r.set_rows_included(anchor.min(ix)..=anchor.max(ix), include, cx));
                }
            })
    }

    /// Thin strip left of the gutter spanning each hunk (Desktop's hunk
    /// handle): hover highlights the hunk, click (de)selects all of it.
    pub fn hunk_handle(&self, diff: &Diff, ix: usize, theme: &Theme) -> Stateful<Div> {
        let hunk = hunk_of(diff, ix);
        let hovered = self.hovered_hunk == Some(hunk);
        let (repo, ws) = (self.repo.clone(), self.ws.clone());
        let ws_hover = self.ws.clone();
        div()
            .id(("hunk-handle", ix))
            .flex_none()
            .w(px(12.0))
            .h_full()
            .flex()
            .justify_center()
            .cursor_pointer()
            .child(div().w(px(4.0)).h_full().bg(if hovered { theme.accent } else { ink(0.07) }))
            .on_hover(move |&on, _, cx| {
                ws_hover.update(cx, |w, cx| {
                    let next = if on { Some(hunk) } else { w.diff_select.hovered_hunk.filter(|&h| h != hunk) };
                    if w.diff_select.hovered_hunk != next {
                        w.diff_select.hovered_hunk = next;
                        cx.notify();
                    }
                })
            })
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                ws.update(cx, |w, _| w.diff_select.drag = None);
                repo.update(cx, |r, cx| r.toggle_hunk(hunk, cx));
            })
    }

    /// Tri-state checkbox shown in a hunk header's gutter.
    pub fn hunk_checkbox(&self, diff: &Diff, ix: usize, r: &RepoModel) -> Checkbox {
        let hunk = hunk_of(diff, ix);
        let repo = self.repo.clone();
        let state: CheckState = r.hunk_state(hunk);
        Checkbox::new(("hunk-check", ix), state)
            .on_click(move |_, _, cx: &mut App| repo.update(cx, |r, cx| r.toggle_hunk(hunk, cx)))
    }
}

impl Workspace {
    pub fn render_diff(
        &mut self,
        diff: Option<Arc<Diff>>,
        highlight: Option<Arc<DiffHighlight>>,
        key: SharedString,
        select: Option<Entity<RepoModel>>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = Theme::of(cx).clone();
        // New file/commit selected: start at the top-left. A reload of the same
        // file (same key) keeps the scroll position.
        if self.diff_key.as_ref() != Some(&key) {
            self.diff_key = Some(key.clone());
            self.diff_scroll.0.borrow().base_handle.set_offset(point(px(0.0), px(0.0)));
            self.diff_select = DiffSelectUi::default();
        }
        let Some(diff) = diff else {
            return div().flex_1().into_any_element();
        };
        let hide_ws = self.settings.diff_ignore_whitespace;
        let message = if diff.is_binary {
            Some("Binary file not shown")
        } else if diff.lines.is_empty() {
            Some(if hide_ws {
                "No content changes (only whitespace differs, or a mode change)"
            } else {
                "No content changes (mode change or empty file)"
            })
        } else {
            None
        };
        if let Some(message) = message {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.text_faint)
                .child(message)
                .into_any_element();
        }
        let note = self.whitespace_note(cx);
        let select = select.filter(|r| r.read(cx).can_select_lines()).map(|repo| RowSelect {
            repo,
            ws: cx.entity(),
            hovered_hunk: self.diff_select.hovered_hunk,
        });
        if self.settings.diff_split {
            let split = self.split_for(&diff);
            let list = super::diff_split::split_list(
                split,
                highlight,
                select,
                &self.diff_scroll,
                theme.font_mono.clone(),
            );
            return self.diff_body(key, list, diff.truncated, note, &theme, cx.entity());
        }
        let n = diff.lines.len();
        let truncated = diff.truncated;
        let longest = diff.longest_line;
        let gutter_digits = diff
            .lines
            .iter()
            .filter_map(|l| l.new_line.max(l.old_line))
            .max()
            .unwrap_or(1)
            .to_string()
            .len()
            .max(3);
        let gutter_w = px(gutter_digits as f32 * 7.5 + 16.0);
        let list = uniform_list("diff-lines", n, move |range, _w, cx| {
            let theme = Theme::of(cx).clone();
            let font = gpui::font(theme.font_mono.clone());
            let r = select.as_ref().map(|s| s.repo.read(cx));
            range
                .map(|ix| {
                    let line = &diff.lines[ix];
                    let (bg, fg, marker) = match line.kind {
                        DiffLineKind::Add => (theme.diff_add_bg, theme.text, "+"),
                        DiffLineKind::Delete => (theme.diff_del_bg, theme.text, "-"),
                        DiffLineKind::Hunk => (theme.diff_hunk_bg, theme.text_faint, ""),
                        DiffLineKind::NoNewline => (gpui::transparent_black(), theme.text_faint, ""),
                        DiffLineKind::Context => (gpui::transparent_black(), theme.text_muted, " "),
                    };
                    let marker_color = match line.kind {
                        DiffLineKind::Add => theme.diff_add,
                        DiffLineKind::Delete => theme.diff_del,
                        _ => theme.text_faint,
                    };
                    let emph_bg = if line.kind == DiffLineKind::Add { theme.diff_add_emph } else { theme.diff_del_emph };
                    let spans = highlight.as_ref().map(|h| h.row(ix)).unwrap_or(&[]);
                    let text = line_text(&line.text, spans, &line.emphasis, fg, emph_bg, &font, &theme.syntax);
                    let hunk = line.kind == DiffLineKind::Hunk;
                    let num = |n: Option<u32>| n.map(|n| n.to_string()).unwrap_or_default();
                    let number = |n: Option<u32>, color: Hsla| {
                        div().flex_none().w(gutter_w).pr(px(8.0)).flex().justify_end().text_color(color).child(num(n))
                    };
                    let row = div()
                        .flex()
                        .h(px(DIFF_ROW_HEIGHT))
                        .w_full()
                        .bg(bg)
                        .whitespace_nowrap()
                        .when(hunk && ix > 0, |d| d.border_t_1().border_color(theme.border))
                        .when(hunk, |d| d.text_color(theme.text_faint).italic());
                    let row = match (&select, r) {
                        (Some(sel), Some(r)) => {
                            let gutter = if hunk {
                                // Checkbox centered over both number columns.
                                div()
                                    .flex()
                                    .flex_none()
                                    .w(gutter_w * 2.0)
                                    .h_full()
                                    .items_center()
                                    .justify_center()
                                    .child(sel.hunk_checkbox(&diff, ix, r))
                                    .into_any_element()
                            } else {
                                let color = if gd_git::selectable(&diff, ix) && r.row_included(ix) {
                                    theme.text
                                } else {
                                    theme.diff_gutter
                                };
                                sel.gutter(&diff, ix, "gutter", false, r, &theme)
                                    .child(number(line.old_line, color))
                                    .child(number(line.new_line, color))
                                    .into_any_element()
                            };
                            row.child(sel.hunk_handle(&diff, ix, &theme))
                                .child(gutter)
                                .child(div().flex_none().w(px(1.0)).h_full().bg(theme.border))
                        }
                        _ => row
                            .child(number(line.old_line, theme.diff_gutter))
                            .child(number(line.new_line, theme.diff_gutter).border_r_1().border_color(theme.border)),
                    };
                    row
                        .child(
                            div()
                                .flex_none()
                                .w(px(20.0))
                                .flex()
                                .justify_center()
                                .text_color(marker_color)
                                .child(marker),
                        )
                        .child(text)
                })
                .collect()
        })
        .with_horizontal_sizing_behavior(gpui::ListHorizontalSizingBehavior::Unconstrained)
        // The list measures ONE row to size the horizontal scroll extent;
        // measure the widest, not row 0 (a short hunk header).
        .with_width_from_item(Some(longest))
        .track_scroll(&self.diff_scroll)
        .size_full()
        .font_family(theme.font_mono.clone())
        .text_size(px(12.0))
        .line_height(px(DIFF_ROW_HEIGHT));
        self.diff_body(key, list.into_any_element(), truncated, note, &theme, cx.entity())
    }

    /// Shared frame around either diff list: whitespace note on top, truncation warning below.
    fn diff_body(
        &self,
        key: SharedString,
        list: gpui::AnyElement,
        truncated: bool,
        note: Option<impl IntoElement>,
        theme: &Theme,
        ws: Entity<Workspace>,
    ) -> gpui::AnyElement {
        motion::fade_quick(
            key,
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .children(note)
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        // A gutter drag ends wherever the button is released.
                        .on_mouse_up_all(move |_, _, _, _, cx| {
                            ws.update(cx, |w, _| w.diff_select.drag = None);
                        })
                        .child(list),
                )
                .when(truncated, |d| {
                    d.child(
                        div()
                            .flex_none()
                            .px(px(16.0))
                            .py(px(6.0))
                            .border_t_1()
                            .border_color(theme.border)
                            .text_size(px(12.0))
                            .text_color(theme.warning)
                            .child("This diff is too large to show completely; it was truncated."),
                    )
                }),
        )
        .into_any_element()
    }
}

