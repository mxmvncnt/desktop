//! Virtualized unified diff. Each row is one `DiffLine`; only visible rows
//! are built per frame, so a 50k-line diff costs the same as a 50-line one.
//!
//! Long lines scroll horizontally by hand ([`TextFlow::Scroll`]): the lists
//! only scroll vertically, and each row shifts its text cell(s) by
//! `DiffView::hscroll`, so gutters stay put and split columns move together.
//! With "Wrap long lines" on, rows wrap and the list becomes a variable-height
//! `list` ([`rows_list`]).

use std::ops::Range;
use std::sync::Arc;

use crate::diff_highlight::{DiffHighlight, line_text};
use crate::repo::RepoModel;
use gd_git::{Diff, DiffLineKind};
use gd_ui::{CheckState, Icon, Theme, icon, motion, theme::{DIFF_ROW_HEIGHT, ink}};
use gpui::{
    AnyElement, App, Context, Div, Entity, Hsla, IntoElement, ListState, MouseButton, ParentElement, Pixels, SharedString, Styled,
    TextRun, UniformListScrollHandle, Window, div, list, point, prelude::*, px, uniform_list,
};

use crate::workspace::Workspace;

/// Pointer state of the line-selection gutter (see `line_selection.rs`).
#[derive(Default)]
pub struct DiffSelectUi {
    /// Handle cell under the pointer as `(block start row, cell id)`: that
    /// change block's handle lights up. The cell id makes leave/enter order
    /// between two cells of the same block harmless.
    pub hovered_handle: Option<(usize, usize)>,
    /// Gutter drag: the row it started on and whether it is including.
    pub drag: Option<(usize, bool)>,
}

/// Everything a row needs to draw and handle the selection gutter.
#[derive(Clone)]
pub struct RowSelect {
    pub repo: Entity<RepoModel>,
    pub ws: Entity<Workspace>,
    /// First row of the change block whose handle is hovered.
    pub hovered_block: Option<usize>,
}

/// Width of the check mark slot in a selectable number cell.
pub const CHECK_W: f32 = 16.0;
/// Width of the hunk handle column.
pub const HANDLE_W: f32 = 16.0;

/// Background of the line-number cells.
fn gutter_bg() -> Hsla {
    ink(0.025)
}

/// Background of the idle handle column: clearly darker than the numbers.
fn handle_bg() -> Hsla {
    gpui::ColorExt::opacity(&gpui::black(), 0.6)
}

/// Background of an included line's number cell (blue-700).
fn line_selected_bg() -> Hsla {
    gd_ui::theme::oklch(0.488, 0.243, 264.376)
}

fn check_mark(color: Hsla) -> impl IntoElement {
    icon(Icon::Check).size(px(11.0)).text_color(color)
}

/// One line-number cell holding `numbers` (one per column: two in unified
/// view, one per side in split view). Without `select` (or for a line that
/// can't be selected) it is plain; otherwise clicking toggles `row`, dragging
/// extends, and an included row is solid blue with one check mark, like Desktop.
pub fn number_cell(
    select: Option<(&RowSelect, &RepoModel)>,
    diff: &Diff,
    row: Option<usize>,
    numbers: &[Option<u32>],
    id: (&'static str, usize),
    num_w: Pixels,
    theme: &Theme,
) -> AnyElement {
    let tint = match row.map(|ix| diff.lines[ix].kind) {
        Some(DiffLineKind::Add) => theme.diff_add_bg,
        Some(DiffLineKind::Delete) => theme.diff_del_bg,
        _ => gutter_bg(),
    };
    let selectable = row.filter(|&ix| gd_git::selectable(diff, ix));
    let included = select.zip(selectable).is_some_and(|((_, r), ix)| r.row_included(ix));
    // Top-aligned so numbers stay on the first line of a wrapped row. No
    // `h_full`: a percentage can't resolve in an auto-height (wrapped) row,
    // and it would stop the cell from stretching to the row's height.
    let cell = div()
        .flex()
        .flex_none()
        .items_start()
        .text_color(theme.diff_gutter)
        // Keep numbers aligned with selectable rows' check slot.
        .when(select.is_some(), |d| {
            d.child(
                div()
                    .flex()
                    .flex_none()
                    .w(px(CHECK_W))
                    .h(px(DIFF_ROW_HEIGHT))
                    .items_center()
                    .justify_center()
                    .when(included, |d| d.child(check_mark(theme.on_accent))),
            )
        })
        .children(numbers.iter().map(|n| {
            div()
                .flex()
                .flex_none()
                .w(num_w)
                .pr(px(8.0))
                .justify_end()
                .child(n.map(|n| n.to_string()).unwrap_or_default())
        }));
    let (Some((sel, _)), Some(ix)) = (select, selectable) else {
        return cell.bg(tint).into_any_element();
    };
    let (repo_down, ws_down) = (sel.repo.clone(), sel.ws.clone());
    let (repo_move, ws_move) = (sel.repo.clone(), sel.ws.clone());
    cell.id(id)
        .cursor_pointer()
        .map(|d| {
            if included {
                // A step darker than the chunk handle so lines read apart from it.
                let hover = theme.accent_strong;
                d.bg(line_selected_bg()).text_color(theme.on_accent).hover_style(move |s| s.bg(hover))
            } else {
                let hover = theme.text_muted;
                d.bg(tint).hover_style(move |s| s.bg(ink(0.14)).text_color(hover))
            }
        })
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
        .into_any_element()
}

impl RowSelect {
    /// One cell of the hunk handle column. Each change block (run of
    /// adds/deletes between unchanged lines) is one handle: blue while any of
    /// its lines is included, one mark on its first row (check = all, dash =
    /// some), hover lights up that block only, click (de)selects the block.
    /// Rows outside a block (`block == None`) show the plain dark strip.
    pub fn handle_cell(
        &self,
        block: Option<Range<usize>>,
        first: bool,
        cell: usize,
        r: &RepoModel,
        theme: &Theme,
    ) -> AnyElement {
        let base = div().flex().flex_none().w(px(HANDLE_W));
        let Some(rows) = block else {
            return base.bg(handle_bg()).into_any_element();
        };
        let start = rows.start;
        let hovered = self.hovered_block == Some(start);
        let state = r.block_state(rows.clone());
        let (repo, ws) = (self.repo.clone(), self.ws.clone());
        let ws_hover = self.ws.clone();
        base.id(("hunk-handle", cell))
            .cursor_pointer()
            .bg(match (state, hovered) {
                (CheckState::Unchecked, false) => ink(0.10),
                (CheckState::Unchecked, true) => ink(0.20),
                (_, false) => theme.accent_strong,
                (_, true) => theme.accent,
            })
            .when(first, |d| {
                // In a box one line tall: a wrapped first row keeps its mark at the top.
                let mark = div().flex().flex_1().h(px(DIFF_ROW_HEIGHT)).items_center().justify_center();
                match state {
                    CheckState::Checked => d.child(mark.child(check_mark(theme.on_accent))),
                    CheckState::Mixed => {
                        d.child(mark.child(div().w(px(7.0)).h(px(1.5)).rounded(px(1.0)).bg(theme.on_accent)))
                    }
                    CheckState::Unchecked => d,
                }
            })
            .on_hover(move |&on, _, cx| {
                ws_hover.update(cx, |w, cx| {
                    let next = if on {
                        Some((start, cell))
                    } else {
                        w.diff_select.hovered_handle.filter(|&(_, c)| c != cell)
                    };
                    if w.diff_select.hovered_handle != next {
                        w.diff_select.hovered_handle = next;
                        cx.notify();
                    }
                })
            })
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                ws.update(cx, |w, _| w.diff_select.drag = None);
                repo.update(cx, |r, cx| r.toggle_block(rows.clone(), cx));
            })
            .into_any_element()
    }
}

/// Which pane a diff is shown in: decides line selection and expansion.
pub enum DiffPane {
    /// Working-tree diff: lines can be selected and hunks expanded.
    Changes(Entity<RepoModel>),
    /// Commit diff: hunks can be expanded.
    History(Entity<RepoModel>),
    /// Stash diff: view only.
    ReadOnly,
}

/// Hunk expansion buttons (Desktop's expand arrows in the hunk header gutter).
#[derive(Clone)]
pub struct Expander {
    pub repo: Entity<RepoModel>,
    /// Expands `RepoModel::commit_diff` instead of `change_diff`.
    pub commit: bool,
}

impl Expander {
    /// Whether a trailing "expand down" row follows the last hunk.
    pub fn has_trailing(diff: &Diff) -> bool {
        diff.hunks.len().checked_sub(1).is_some_and(|last| gd_git::gap_below(diff, last) > 0)
    }

    /// Buttons for hunk header `h`, or for the trailing row (`h == hunks.len()`):
    /// first hunk: up; between hunks: "expand all" when the gap is at most one
    /// step, else down (previous hunk) + up; trailing row: down.
    fn buttons(&self, diff: &Diff, h: usize, theme: &Theme) -> Vec<AnyElement> {
        use gd_git::Expansion::{All, Down, Up};
        let n = diff.hunks.len();
        let mut out = Vec::new();
        let mut add = |k: usize, ic: Icon, tip: &'static str, e| out.push(self.button(h * 2 + k, ic, tip, e, theme));
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

    fn button(&self, id: usize, ic: Icon, tip: &'static str, e: gd_git::Expansion, theme: &Theme) -> AnyElement {
        let (repo, commit) = (self.repo.clone(), self.commit);
        let group = SharedString::from(format!("expand-{id}"));
        let hover_fg = theme.on_accent;
        let hover_bg = theme.accent_strong;
        div()
            .id(("expand", id))
            .group(group.clone())
            .flex()
            .flex_1()
            .h(px(DIFF_ROW_HEIGHT))
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover_style(move |s| s.bg(hover_bg))
            .tooltip(gd_ui::tooltip(tip))
            .child(
                icon(ic)
                    .size(px(12.0))
                    .text_color(theme.text_faint)
                    .group_hover_style(group, move |s| s.text_color(hover_fg)),
            )
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                repo.update(cx, |r, cx| r.expand_diff(commit, e, cx));
            })
            .into_any_element()
    }

    /// The gutter of a hunk header (or trailing) row, `width` wide.
    pub fn gutter(exp: Option<&Self>, diff: &Diff, h: usize, width: Pixels, theme: &Theme) -> Div {
        div()
            .flex()
            .flex_none()
            .w(width)
            .bg(handle_bg())
            .children(exp.map(|e| e.buttons(diff, h, theme)).unwrap_or_default())
    }
}

/// The trailing row after the last hunk: an "expand down" gutter.
pub fn trailing_row(exp: Option<&Expander>, diff: &Diff, width: Pixels, flow: TextFlow, theme: &Theme) -> AnyElement {
    row_box(flow)
        .bg(theme.diff_hunk_bg)
        .border_t_1()
        .border_color(theme.border)
        .child(Expander::gutter(exp, diff, diff.hunks.len(), width, theme).border_r_1().border_color(theme.border))
        .into_any_element()
}

/// Hunk index of header row `ix`.
pub fn header_hunk(diff: &Diff, ix: usize) -> Option<usize> {
    diff.hunks.binary_search_by_key(&ix, |h| h.header_index).ok()
}

/// How row text is laid out. `Scroll` clips each text cell and shifts it
/// left by the shared horizontal offset (the same value for both split
/// columns, so they always move together); `Wrap` breaks long lines.
#[derive(Clone, Copy, PartialEq)]
pub enum TextFlow {
    Scroll(Pixels),
    Wrap,
}

/// Width of the `+`/`-` marker column.
pub const MARKER_W: f32 = 20.0;

/// The box of one diff row: one line tall, or at least one line when wrapping.
pub fn row_box(flow: TextFlow) -> Div {
    let d = div().flex().w_full();
    match flow {
        TextFlow::Scroll(_) => d.h(px(DIFF_ROW_HEIGHT)).whitespace_nowrap(),
        TextFlow::Wrap => d.min_h(px(DIFF_ROW_HEIGHT)),
    }
}

/// The text part of a row (after the marker), see [`TextFlow`].
pub fn text_cell(text: AnyElement, flow: TextFlow) -> Div {
    let cell = div().flex_1().min_w_0();
    match flow {
        TextFlow::Scroll(x) => cell.h_full().overflow_hidden().child(div().relative().left(-x).child(text)),
        TextFlow::Wrap => cell.whitespace_normal().child(text),
    }
}

/// The `+`/`-` marker column.
pub fn marker_cell(marker: &'static str, color: Hsla) -> Div {
    div().flex_none().w(px(MARKER_W)).flex().justify_center().text_color(color).child(marker)
}

/// The virtualized list around a row renderer: a `uniform_list` when rows are
/// one line tall, or (`wrap`) a `list` whose rows measure their own height.
pub fn rows_list(
    id: &'static str,
    n: usize,
    wrap: Option<ListState>,
    scroll: &UniformListScrollHandle,
    mono: SharedString,
    render: impl Fn(usize, &Theme, &App) -> AnyElement + 'static,
) -> AnyElement {
    match wrap {
        Some(state) => list(state, move |ix, _w, cx| render(ix, Theme::of(cx), cx))
            .size_full()
            .font_family(mono)
            .text_size(px(12.0))
            .line_height(px(DIFF_ROW_HEIGHT))
            .into_any_element(),
        None => {
            let mut list = uniform_list(id, n, move |range, _w, cx| {
                let theme = Theme::of(cx);
                range.map(|ix| render(ix, theme, cx)).collect()
            })
            .track_scroll(scroll)
            .size_full()
            .font_family(mono)
            .text_size(px(12.0))
            .line_height(px(DIFF_ROW_HEIGHT));
            // Horizontal wheel deltas belong to `Workspace::scroll_diff_x`;
            // without this the list turns them into vertical scrolling.
            list.interactivity().base_style.restrict_scroll_to_axis = Some(true);
            list.into_any_element()
        }
    }
}

/// One row of the unified diff: `[handle][check · old · new][marker][text]`.
#[allow(clippy::too_many_arguments)]
fn unified_row(
    diff: &Diff,
    hl: Option<&DiffHighlight>,
    sel: Option<(&RowSelect, &RepoModel)>,
    exp: Option<&Expander>,
    ix: usize,
    num_w: Pixels,
    flow: TextFlow,
    theme: &Theme,
) -> AnyElement {
    let gutter_w = number_cell_w(num_w, 2.0, sel.is_some()) + if sel.is_some() { px(HANDLE_W) } else { px(0.0) };
    if ix == diff.lines.len() {
        return trailing_row(exp, diff, gutter_w, flow, theme);
    }
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
    let font = gpui::font(theme.font_mono.clone());
    let emph_bg = if line.kind == DiffLineKind::Add { theme.diff_add_emph } else { theme.diff_del_emph };
    let spans = hl.map(|h| h.row(ix)).unwrap_or(&[]);
    let text = line_text(&line.text, spans, &line.emphasis, fg, emph_bg, &font, &theme.syntax);
    let hunk = line.kind == DiffLineKind::Hunk;
    let row = row_box(flow)
        .bg(bg)
        .when(hunk && ix > 0, |d| d.border_t_1().border_color(theme.border))
        .when(hunk, |d| d.text_color(theme.text_faint).italic());
    if let Some(h) = header_hunk(diff, ix) {
        // Hunk headers carry the expand buttons in their gutter.
        return row
            .child(Expander::gutter(exp, diff, h, gutter_w, theme).border_r_1().border_color(theme.border))
            .child(marker_cell("", marker_color))
            .child(text_cell(text, flow))
            .into_any_element();
    }
    let line_row = Some(ix);
    row
        // One selection cell per line, like Desktop's unified view.
        .when_some(sel, |d, (s, r)| {
            let block = RepoModel::change_block(diff, ix);
            let first = block.as_ref().is_some_and(|b| b.start == ix);
            d.child(s.handle_cell(block, first, ix, r, theme))
        })
        .child(div().flex().flex_none().border_r_1().border_color(theme.border).child(number_cell(
            sel,
            diff,
            line_row,
            &[line.old_line, line.new_line],
            ("line-no", ix),
            num_w,
            theme,
        )))
        .child(marker_cell(marker, marker_color))
        .child(text_cell(text, flow))
        .into_any_element()
}

/// Width of a number cell holding `numbers` numbers.
pub fn number_cell_w(num_w: Pixels, numbers: f32, select: bool) -> Pixels {
    num_w * numbers + if select { px(CHECK_W) } else { px(0.0) }
}

impl Workspace {
    pub fn render_diff(
        &mut self,
        diff: Option<Arc<Diff>>,
        highlight: Option<Arc<DiffHighlight>>,
        key: SharedString,
        pane: DiffPane,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = Theme::of(cx).clone();
        // New file/commit selected: start at the top-left. A reload of the same
        // file (same key) keeps the scroll position.
        let new_key = self.diff_key.as_ref() != Some(&key);
        if new_key {
            self.diff_key = Some(key.clone());
            self.diff_scroll.0.borrow().base_handle.set_offset(point(px(0.0), px(0.0)));
            self.diff_view.hscroll = px(0.0);
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
        let (select, exp) = match pane {
            DiffPane::Changes(repo) => (Some(repo.clone()), Some(Expander { repo, commit: false })),
            DiffPane::History(repo) => (None, Some(Expander { repo, commit: true })),
            DiffPane::ReadOnly => (None, None),
        };
        let exp = exp.filter(|_| gd_git::expandable(&diff));
        // One extra row after the last hunk for "expand down".
        let trailing = usize::from(exp.is_some() && Expander::has_trailing(&diff));
        let select = select.filter(|r| r.read(cx).can_select_lines()).map(|repo| RowSelect {
            repo,
            ws: cx.entity(),
            hovered_block: self.diff_select.hovered_handle.map(|(b, _)| b),
        });
        let has_select = select.is_some();
        let wrap = self.settings.diff_wrap;
        let flow = if wrap { TextFlow::Wrap } else { TextFlow::Scroll(self.diff_view.hscroll) };
        let split = self.settings.diff_split.then(|| self.split_for(&diff));
        let n = split.as_ref().map_or(diff.lines.len(), |s| s.rows.len()) + trailing;
        let wrap_list = wrap.then(|| self.diff_view.wrap_list_for(&diff, split.is_some(), n, new_key));
        let truncated = diff.truncated;
        let mono = theme.font_mono.clone();
        let list = if let Some(split) = split {
            let num_w = px(split.gutter_digits as f32 * 7.5 + 16.0);
            // Middle gutter (two number cells, the handle, two borders) and two markers.
            let fixed = number_cell_w(num_w, 1.0, has_select) * 2.0
                + if has_select { px(HANDLE_W) } else { px(0.0) }
                + px(2.0 + 2.0 * MARKER_W);
            self.diff_view.set_extent(&diff, fixed, 2.0);
            super::diff_split::split_list(split, highlight, select, exp, trailing == 1, &self.diff_scroll, wrap_list, flow, mono)
        } else {
            let gutter_digits = diff
                .lines
                .iter()
                .filter_map(|l| l.new_line.max(l.old_line))
                .max()
                .unwrap_or(1)
                .to_string()
                .len()
                .max(3);
            let num_w = px(gutter_digits as f32 * 7.5 + 16.0);
            let fixed = number_cell_w(num_w, 2.0, has_select)
                + if has_select { px(HANDLE_W) } else { px(0.0) }
                + px(1.0 + MARKER_W);
            self.diff_view.set_extent(&diff, fixed, 1.0);
            rows_list("diff-lines", n, wrap_list, &self.diff_scroll, mono, move |ix, theme, cx| {
                let r = select.as_ref().map(|s| s.repo.read(cx));
                let sel = select.as_ref().zip(r);
                unified_row(&diff, highlight.as_deref(), sel, exp.as_ref(), ix, num_w, flow, theme)
            })
        };
        self.diff_body(key, list, truncated, wrap, note, &theme, cx.entity())
    }

    /// Wheel/trackpad horizontal scroll of the (non-wrapped) diff, shared by
    /// both split columns. Clamped so the widest line can just be read to its end.
    pub fn scroll_diff_x(&mut self, by: Pixels, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ext) = &self.diff_view.extent else { return };
        let Some(line) = ext.diff.lines.get(ext.diff.longest_line).filter(|l| !l.text.is_empty()) else {
            return;
        };
        let run = TextRun {
            len: line.text.len(),
            font: gpui::font(Theme::of(cx).font_mono.clone()),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
            letter_spacing: None,
        };
        let width = window.text_system().shape_line(line.text.clone().into(), px(12.0), &[run]).width;
        let view = self.diff_scroll.0.borrow().base_handle.bounds().size.width;
        let visible = (view - ext.fixed) / ext.columns;
        let max = (width + px(24.0) - visible).max(px(0.0));
        let next = (self.diff_view.hscroll + by).min(max).max(px(0.0));
        if next != self.diff_view.hscroll {
            self.diff_view.hscroll = next;
            cx.notify();
        }
    }

    /// Shared frame around either diff list: whitespace note on top, truncation warning below.
    #[allow(clippy::too_many_arguments)]
    fn diff_body(
        &self,
        key: SharedString,
        list: gpui::AnyElement,
        truncated: bool,
        wrap: bool,
        note: Option<impl IntoElement>,
        theme: &Theme,
        ws: Entity<Workspace>,
    ) -> gpui::AnyElement {
        let ws_wheel = ws.clone();
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
                        // Shift+wheel arrives as a horizontal delta on every platform.
                        .when(!wrap, |d| {
                            d.on_scroll_wheel(move |e, window, cx| {
                                let dx = e.delta.pixel_delta(window.line_height()).x;
                                if dx != px(0.0) {
                                    ws_wheel.update(cx, |w, cx| w.scroll_diff_x(-dx, window, cx));
                                }
                            })
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
