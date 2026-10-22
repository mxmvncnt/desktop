//! Small note shown under the diff header while whitespace changes are hidden.

use gd_ui::{Icon, Theme, icon};
use gpui::{App, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};

#[derive(IntoElement)]
pub struct WhitespaceNote;

impl WhitespaceNote {
    /// `None` unless whitespace changes are hidden.
    pub fn new(ignore_whitespace: bool) -> Option<Self> {
        ignore_whitespace.then_some(Self)
    }
}

impl RenderOnce for WhitespaceNote {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx);
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.0))
            .h(px(24.0))
            .px(px(16.0))
            .border_b_1()
            .border_color(theme.border)
            .bg(gd_ui::theme::ink(0.02))
            .text_size(px(12.0))
            .text_color(theme.text_faint)
            .child(icon(Icon::EyeClosed).size(px(13.0)).text_color(theme.text_faint))
            .child("Whitespace changes are hidden")
    }
}
