//! Mouse wiring of the diff text selection. Plain fns on a small props struct
//! (hot-row exception, see docs/CONVENTIONS.md): the row renderers call
//! `bind_text` / `bind_drag` for every visible row. The model is in
//! `text_selection.rs`, the workspace-side handlers in `text_handlers.rs`.

use std::ops::Range;

use gpui::{Div, Entity, Hsla, InteractiveElement, MouseButton, Pixels, Point, Styled, TextLayout};

use super::text_selection::{TextPos, TextSel, TextSide};
use crate::workspace::Workspace;

/// Selection state and mouse wiring handed to every row renderer.
#[derive(Clone)]
pub struct TextSelect {
    pub ws: Entity<Workspace>,
    pub sel: Option<TextSel>,
    /// Highlight color (`Theme::selection`).
    pub color: Hsla,
}

/// One selectable text cell of a row, with its laid-out text for hit-testing.
#[derive(Clone)]
pub struct TextTarget {
    pub line: usize,
    pub side: TextSide,
    pub layout: TextLayout,
}

impl TextSelect {
    /// Selected byte range of `line`'s text (`len` bytes) when shown on `side`.
    pub fn cols(&self, line: usize, side: TextSide, len: usize) -> Option<Range<usize>> {
        self.sel.and_then(|s| s.cols_in(line, side, len))
    }

    /// Pressing on the text of a row starts (or with shift extends) a selection.
    pub fn bind_text(&self, cell: Div, target: TextTarget) -> Div {
        let ws = self.ws.clone();
        cell.cursor_text().on_mouse_down(MouseButton::Left, move |e, window, cx| {
            let pos = TextPos { line: target.line, col: col_at(&target.layout, e.position) };
            let (side, clicks, shift) = (target.side, e.click_count, e.modifiers.shift);
            ws.update(cx, |w, cx| w.diff_text_down(pos, side, clicks, shift, window, cx));
        })
    }

    /// While dragging, the pointer over any part of the row moves the caret
    /// to the nearest position in the text column the drag started in.
    pub fn bind_drag(&self, row: Div, targets: Vec<TextTarget>) -> Div {
        let ws = self.ws.clone();
        row.on_mouse_move(move |e, _, cx| {
            if e.pressed_button != Some(MouseButton::Left) {
                return;
            }
            let Some(side) = ws.read(cx).diff_view.dragging_side() else { return };
            let Some(t) = targets.iter().find(|t| t.side == side) else { return };
            let pos = TextPos { line: t.line, col: col_at(&t.layout, e.position) };
            ws.update(cx, |w, cx| w.diff_text_drag(pos, cx));
        })
    }
}

/// Byte offset nearest to `at` (window coordinates) in the laid-out text.
fn col_at(layout: &TextLayout, at: Point<Pixels>) -> usize {
    layout.index_for_position(at).unwrap_or_else(|nearest| nearest)
}
