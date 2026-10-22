//! "Clone a repository" popover.

use gd_ui::{Button, Theme};
use gpui::{App, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};
use gpui_elements::editable_text::{EditableTextState, actions::Enter};

use crate::workspace::{Workspace, text_field};

#[derive(IntoElement)]
pub struct CloneForm {
    url_empty: bool,
    input: Entity<EditableTextState>,
    ws: Entity<Workspace>,
}

impl CloneForm {
    pub fn new(url_empty: bool, input: Entity<EditableTextState>, ws: Entity<Workspace>) -> Self {
        Self { url_empty, input, ws }
    }
}

impl RenderOnce for CloneForm {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let (enter_ws, click_ws) = (self.ws.clone(), self.ws.clone());
        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .p(px(14.0))
            .capture_action(move |_: &Enter, _w, cx| {
                cx.stop_propagation();
                enter_ws.update(cx, |this, cx| this.submit_clone(cx))
            })
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text_muted)
                    .child("Clone a repository"),
            )
            .child(text_field("clone-url", &self.input, "https://github.com/owner/repo.git", cx))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme.text_faint)
                    .child("You'll pick the parent folder next."),
            )
            .child(
                Button::new("do-clone")
                    .primary()
                    .full_width()
                    .label("Clone")
                    .disabled(self.url_empty)
                    .on_click(move |_, _, cx| click_ws.update(cx, |this, cx| this.submit_clone(cx))),
            )
    }
}
