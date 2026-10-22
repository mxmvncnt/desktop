//! "Create a branch" popover.

use gd_ui::{Button, Theme};
use gpui::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};
use gpui_elements::editable_text::{EditableTextState, actions::Enter};

use crate::workspace::{Workspace, text_field};

#[derive(IntoElement)]
pub struct NewBranchForm {
    /// Sanitized text of the name input.
    name: String,
    base: String,
    input: Entity<EditableTextState>,
    ws: Entity<Workspace>,
}

impl NewBranchForm {
    pub fn new(name: String, base: String, input: Entity<EditableTextState>, ws: Entity<Workspace>) -> Self {
        Self { name, base, input, ws }
    }

    fn hint(&self) -> SharedString {
        if self.name.is_empty() {
            SharedString::from(format!("Based on {}", self.base))
        } else {
            format!("Creates {} from {} and switches to it", self.name, self.base).into()
        }
    }
}

impl RenderOnce for NewBranchForm {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let (enter_ws, click_ws) = (self.ws.clone(), self.ws.clone());
        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .p(px(14.0))
            .capture_action(move |_: &Enter, w, cx| {
                cx.stop_propagation();
                enter_ws.update(cx, |this, cx| this.submit_new_branch(w, cx))
            })
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text_muted)
                    .child("Create a branch"),
            )
            .child(text_field("branch-name", &self.input, "Branch name", cx))
            .child(div().text_size(px(12.0)).text_color(theme.text_faint).child(self.hint()))
            .child(
                Button::new("create-branch")
                    .primary()
                    .full_width()
                    .label("Create branch")
                    .disabled(self.name.is_empty())
                    .on_click(move |_, w, cx| click_ws.update(cx, |this, cx| this.submit_new_branch(w, cx))),
            )
    }
}
