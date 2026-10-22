//! How row text is laid out (scroll vs wrap) and the row/text/marker boxes
//! shared by the unified and split row renderers.

use gd_ui::theme::DIFF_ROW_HEIGHT;
use gpui::{AnyElement, Div, Hsla, ParentElement, Pixels, Styled, div, px};

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
