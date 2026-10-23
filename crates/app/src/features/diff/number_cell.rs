//! One line-number cell of a diff row. A plain fn, not a `RenderOnce`: it is
//! built for every visible row every frame (hot-row exception, see
//! docs/CONVENTIONS.md).

use gd_git::{Diff, DiffLineKind};
use gd_ui::{Theme, theme::{DIFF_ROW_HEIGHT, ink}};
use gpui::{AnyElement, Div, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Styled, div, prelude::*, px};

use super::gutter_style::{CHECK_W, check_mark, gutter_bg, line_selected_bg};
use super::row_select::RowSelect;
use super::state::GutterDrag;
use crate::repo::RepoModel;

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
    let cell = cell_shell(select.is_some(), included, numbers, num_w, theme);
    let (Some((sel, _)), Some(ix)) = (select, selectable) else {
        return cell.bg(tint).into_any_element();
    };
    selectable_cell(cell, sel, ix, id, included, tint, theme)
}

/// The cell box: optional check slot, then one right-aligned number per column.
/// Top-aligned so numbers stay on the first line of a wrapped row. No
/// `h_full`: a percentage can't resolve in an auto-height (wrapped) row,
/// and it would stop the cell from stretching to the row's height.
fn cell_shell(has_select: bool, included: bool, numbers: &[Option<u32>], num_w: Pixels, theme: &Theme) -> Div {
    div()
        .flex()
        .flex_none()
        .items_start()
        .text_color(theme.diff_gutter)
        // Keep numbers aligned with selectable rows' check slot.
        .when(has_select, |d| {
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
        }))
}

/// Click toggles row `ix`; dragging from it extends the (de)selection.
fn selectable_cell(
    cell: Div,
    sel: &RowSelect,
    ix: usize,
    id: (&'static str, usize),
    included: bool,
    tint: gpui::Hsla,
    theme: &Theme,
) -> AnyElement {
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
            ws_down.update(cx, |w, _| w.diff_select.drag = Some(GutterDrag { anchor: ix, include }));
        })
        .on_mouse_move(move |e, _, cx| {
            if e.pressed_button != Some(MouseButton::Left) {
                return;
            }
            if let Some(GutterDrag { anchor, include }) = ws_move.read(cx).diff_select.drag {
                repo_move.update(cx, |r, cx| r.set_rows_included(anchor.min(ix)..=anchor.max(ix), include, cx));
            }
        })
        .into_any_element()
}
