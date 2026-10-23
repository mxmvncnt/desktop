//! A radio-style option card used by the stash and switch-branch dialogs.

use gd_ui::{Theme, pressable};
use gpui::{App, ElementId, IntoElement, RenderOnce, Window, div, prelude::*, px};

#[derive(IntoElement)]
pub struct OptionRow {
    id: ElementId,
    selected: bool,
    title: String,
    detail: Option<String>,
    on_select: Box<dyn Fn(&mut Window, &mut App)>,
}

impl OptionRow {
    pub fn new(
        id: impl Into<ElementId>,
        selected: bool,
        title: impl Into<String>,
        on_select: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self { id: id.into(), selected, title: title.into(), detail: None, on_select: Box::new(on_select) }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl RenderOnce for OptionRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let selected = self.selected;
        let on_select = self.on_select;
        pressable(self.id, selected, cx)
            .flex()
            .items_start()
            .gap(px(10.0))
            .px(px(12.0))
            .py(px(9.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(if selected { theme.border_strong } else { theme.border })
            .child(radio_dot(selected, &theme))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(self.title),
                    )
                    .children(self.detail.map(|d| div().text_size(px(12.0)).text_color(theme.text_faint).child(d))),
            )
            .on_click(move |_, w, cx| on_select(w, cx))
    }
}

fn radio_dot(selected: bool, theme: &Theme) -> impl IntoElement {
    div()
        .flex_none()
        .mt(px(2.0))
        .size(px(14.0))
        .rounded_full()
        .border_1()
        .border_color(if selected { theme.solid } else { theme.border_strong })
        .flex()
        .items_center()
        .justify_center()
        .when(selected, |d| d.child(div().size(px(8.0)).rounded_full().bg(theme.solid)))
}
