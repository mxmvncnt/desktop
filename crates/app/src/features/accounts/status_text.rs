//! A one-line success / error note.

use gd_ui::Theme;
use gpui::{App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px};

use super::state::StatusLine;

#[derive(IntoElement)]
pub struct StatusText {
    text: SharedString,
    error: bool,
}

impl StatusText {
    pub fn new(line: &StatusLine) -> Self {
        Self { text: line.text.clone().into(), error: line.error }
    }

    pub fn error(text: &str) -> Self {
        Self { text: text.to_string().into(), error: true }
    }
}

impl RenderOnce for StatusText {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx);
        let color = if self.error { theme.danger } else { theme.success };
        div().text_size(px(12.0)).text_color(color).child(self.text)
    }
}
