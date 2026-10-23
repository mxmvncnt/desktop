//! Category sidebar of the settings modal.

use gd_ui::{Theme, icon, pressable, theme::ink};
use gpui::{
    App, Entity, IntoElement, ParentElement, RenderOnce, StatefulInteractiveElement, Styled,
    Window, div, px,
};

use super::SettingsPage;
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct SettingsNav {
    page: SettingsPage,
    ws: Entity<Workspace>,
}

impl SettingsNav {
    pub fn new(page: SettingsPage, ws: Entity<Workspace>) -> Self {
        Self { page, ws }
    }

    fn item(&self, p: SettingsPage, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let ws = self.ws.clone();
        let active = p == self.page;
        pressable(p.label(), active, cx)
            .flex()
            .items_center()
            .gap(px(10.0))
            .h(px(30.0))
            .px(px(10.0))
            .rounded(px(gd_ui::theme::CONTROL_RADIUS))
            .text_size(px(13.0))
            .text_color(if active { theme.text } else { theme.text_muted })
            .child(icon(p.icon()).size(px(15.0)).text_color(if active { theme.text } else { theme.text_faint }))
            .child(p.label())
            .on_click(move |_, _, cx| ws.update(cx, |this, cx| this.set_settings_page(p, cx)))
    }
}

impl RenderOnce for SettingsNav {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        SettingsPage::ALL.into_iter().fold(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .p(px(8.0))
                .w(px(188.0))
                .flex_none()
                .border_r_1()
                .border_color(theme.border)
                .bg(ink(0.02)),
            |nav, p| nav.child(self.item(p, cx)),
        )
    }
}
