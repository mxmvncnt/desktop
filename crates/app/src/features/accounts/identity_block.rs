//! One name + email editor with its action buttons and result line
//! (used for the global identity and the repository override).

use gd_ui::{Button, Theme};
use gpui::{App, Entity, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px};
use gpui_elements::editable_text::EditableTextState;

use super::field_row::FieldRow;
use super::state::StatusLine;
use super::status_text::StatusText;

#[derive(IntoElement)]
pub struct IdentityBlock {
    title: SharedString,
    caption: SharedString,
    name_id: &'static str,
    email_id: &'static str,
    name: Entity<EditableTextState>,
    email: Entity<EditableTextState>,
    buttons: Vec<Button>,
    status: Option<StatusLine>,
}

impl IdentityBlock {
    pub fn new(
        title: impl Into<SharedString>,
        caption: impl Into<SharedString>,
        ids: (&'static str, &'static str),
        inputs: (&Entity<EditableTextState>, &Entity<EditableTextState>),
    ) -> Self {
        Self {
            title: title.into(),
            caption: caption.into(),
            name_id: ids.0,
            email_id: ids.1,
            name: inputs.0.clone(),
            email: inputs.1.clone(),
            buttons: Vec::new(),
            status: None,
        }
    }

    pub fn button(mut self, b: Button) -> Self {
        self.buttons.push(b);
        self
    }

    pub fn status(mut self, s: Option<StatusLine>) -> Self {
        self.status = s;
        self
    }
}

impl RenderOnce for IdentityBlock {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(div().font_weight(gpui::FontWeight::MEDIUM).child(self.title))
            .child(div().text_size(px(12.0)).text_color(theme.text_faint).child(self.caption))
            .child(
                div()
                    .flex()
                    .gap(px(10.0))
                    .child(div().flex_1().child(FieldRow::new("Name", self.name_id, &self.name, "Your Name")))
                    .child(div().flex_1().child(FieldRow::new("Email", self.email_id, &self.email, "you@example.com"))),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .children(self.buttons)
                    .children(self.status.as_ref().map(StatusText::new)),
            )
    }
}
