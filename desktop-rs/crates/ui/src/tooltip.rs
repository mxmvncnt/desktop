//! Small zeron-style tooltip for `.tooltip(gd_ui::tooltip("text"))`.

use gpui::{AnyView, App, AppContext, Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, div, px};

use crate::theme::Theme;

pub struct Tooltip {
    text: SharedString,
}

impl Render for Tooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx);
        div()
            .px(px(8.0))
            .py(px(5.0))
            .rounded(px(6.0))
            .bg(theme.surface_overlay)
            .border_1()
            .border_color(theme.border_strong)
            .shadow_md()
            .text_size(px(12.0))
            .text_color(theme.text)
            .child(self.text.clone())
    }
}

/// Builder for `InteractiveElement::tooltip`.
pub fn tooltip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_, cx| cx.new(|_| Tooltip { text: text.clone() }).into()
}
