//! Card with an icon, a title, a one-line body and an action button
//! (welcome screen, "No local changes" suggestions).

use gd_ui::{Button, Icon, Theme, icon};
use gpui::{
    App, ClickEvent, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};

#[derive(IntoElement)]
pub struct SuggestionCard {
    id: &'static str,
    icon: Icon,
    title: SharedString,
    body: SharedString,
    action: SharedString,
    on_click: Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>,
}

impl SuggestionCard {
    pub fn new(
        id: &'static str,
        icon: Icon,
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
        action: impl Into<SharedString>,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id,
            icon,
            title: title.into(),
            body: body.into(),
            action: action.into(),
            on_click: Box::new(on_click),
        }
    }
}

impl RenderOnce for SuggestionCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        div()
            .flex()
            .items_center()
            .gap(px(14.0))
            .p(px(14.0))
            .rounded(px(gd_ui::theme::PANEL_RADIUS))
            .bg(theme.surface_card)
            .border_1()
            .border_color(theme.border)
            .child(
                div()
                    .size(px(32.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.0))
                    .bg(gd_ui::theme::ink(0.06))
                    .child(icon(self.icon).text_color(theme.text_muted)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.0))
                    .child(div().font_weight(gpui::FontWeight::MEDIUM).child(self.title))
                    .child(div().text_size(px(12.0)).text_color(theme.text_faint).truncate().child(self.body)),
            )
            .child(Button::new(self.id).label(self.action).compact().on_click(self.on_click))
    }
}
