//! Shared styling for single-line inputs.

use gd_ui::Theme;
use gpui::{App, Entity, IntoElement, Styled, px};
use gpui_elements::editable_text::EditableTextState;

pub fn text_field(
    id: &'static str,
    state: &Entity<EditableTextState>,
    placeholder: &str,
    cx: &App,
) -> impl IntoElement + use<> {
    let theme = Theme::of(cx);
    gpui_elements::editable_text::text_input(id)
        .state(state.downgrade())
        .placeholder(placeholder.to_string())
        .placeholder_color(theme.text_faint)
        .selection_color(theme.selection)
        .caret_color(theme.caret)
        .caret_blink_interval_500ms()
        .w_full()
        .h(px(32.0))
        .px(px(10.0))
        .py(px(7.0))
        .rounded(px(gd_ui::theme::CONTROL_RADIUS + 1.0))
        .bg(theme.input_bg)
        .border_1()
        .border_color(theme.border)
        .text_size(px(13.0))
        .text_color(theme.text)
        .whitespace_nowrap()
        .overflow_hidden()
}
