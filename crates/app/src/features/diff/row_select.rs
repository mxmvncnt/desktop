//! The hunk handle column of the selection gutter. A plain method, not a
//! `RenderOnce`: one cell is built per visible row per frame (hot-row
//! exception, see docs/CONVENTIONS.md).

use std::ops::Range;

use gd_ui::{CheckState, Theme, theme::{DIFF_ROW_HEIGHT, ink}};
use gpui::{AnyElement, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement, StatefulInteractiveElement, Styled, div, prelude::*, px};

use super::gutter_style::{HANDLE_W, check_mark, handle_bg};
use super::state::HoveredHandle;
use crate::repo::RepoModel;
use crate::workspace::Workspace;

/// Everything a row needs to draw and handle the selection gutter.
#[derive(Clone)]
pub struct RowSelect {
    pub repo: Entity<RepoModel>,
    pub ws: Entity<Workspace>,
    /// First row of the change block whose handle is hovered.
    pub hovered_block: Option<usize>,
}

/// Whether a handle cell sits on the first row of its change block (the one
/// that shows the check/dash mark).
#[derive(Clone, Copy, PartialEq)]
pub enum BlockRow {
    First,
    Rest,
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
        row: BlockRow,
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
            .when(row == BlockRow::First, |d| d.children(handle_mark(state, theme)))
            .on_hover(move |&on, _, cx| {
                ws_hover.update(cx, |w, cx| {
                    let next = if on {
                        Some(HoveredHandle { block: start, cell })
                    } else {
                        w.diff_select.hovered_handle.filter(|h| h.cell != cell)
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

/// The check (all) or dash (some) mark of a block, in a box one line tall: a
/// wrapped first row keeps its mark at the top.
fn handle_mark(state: CheckState, theme: &Theme) -> Option<gpui::Div> {
    let mark = div().flex().flex_1().h(px(DIFF_ROW_HEIGHT)).items_center().justify_center();
    match state {
        CheckState::Checked => Some(mark.child(check_mark(theme.on_accent))),
        CheckState::Mixed => Some(mark.child(div().w(px(7.0)).h(px(1.5)).rounded(px(1.0)).bg(theme.on_accent))),
        CheckState::Unchecked => None,
    }
}
