//! A caption above a single-line input.

use gd_ui::Theme;
use gpui::{App, Entity, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px};
use gpui_elements::editable_text::EditableTextState;

use crate::shared::text_field::text_field;

#[derive(IntoElement)]
pub struct FieldRow {
    label: SharedString,
    id: &'static str,
    state: Entity<EditableTextState>,
    placeholder: SharedString,
}

impl FieldRow {
    pub fn new(label: &'static str, id: &'static str, state: &Entity<EditableTextState>, placeholder: &'static str) -> Self {
        Self { label: label.into(), id, state: state.clone(), placeholder: placeholder.into() }
    }
}

impl RenderOnce for FieldRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(div().text_size(px(12.0)).text_color(theme.text_muted).child(self.label))
            .child(text_field(self.id, &self.state, &self.placeholder, cx))
    }
}
