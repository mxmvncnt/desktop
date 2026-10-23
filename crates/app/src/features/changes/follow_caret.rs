//! Keeps the caret of a commit-box input in view while typing.
//!
//! gpui-ce's editable text only scrolls to the caret on caret movement, using
//! the layout of the previous text version, so typing at the end of a long
//! text never scrolls. After each edit we wait for the new layout and then
//! "move" the caret to where it already is, which scrolls it into view.

use gpui::{Context, Entity, Subscription, Window};
use gpui_elements::editable_text::{EditableTextState, TextChanged};
use gpui::EntityInputHandler;

use crate::workspace::Workspace;

pub fn follow_caret(
    input: &Entity<EditableTextState>,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) -> Subscription {
    cx.subscribe_in(input, window, |_: &mut Workspace, input, _: &TextChanged, window, _| {
        let input = input.clone();
        // Frame N draws the new text (layout); frame N+1 can see its geometry.
        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, cx| {
                input.update(cx, |state, cx| {
                    let Some(sel) = state.selected_text_range(true, window, cx) else { return };
                    let utf16 = if sel.reversed { sel.range.start } else { sel.range.end };
                    state.move_to(utf16_to_byte(state.as_str(), utf16), cx);
                });
            });
        });
    })
}

fn utf16_to_byte(s: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (byte, c) in s.char_indices() {
        if units >= utf16 {
            return byte;
        }
        units += c.len_utf16();
    }
    s.len()
}
