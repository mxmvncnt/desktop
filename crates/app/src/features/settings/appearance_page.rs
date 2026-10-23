use gd_ui::{Tabs, Theme};
use gpui::{App, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};

use super::ThemeMode;
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct AppearancePage {
    mode: ThemeMode,
    ws: Entity<Workspace>,
}

impl AppearancePage {
    pub fn new(mode: ThemeMode, ws: Entity<Workspace>) -> Self {
        Self { mode, ws }
    }
}

impl RenderOnce for AppearancePage {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let ws = self.ws;
        let selected = ThemeMode::ALL.iter().position(|m| *m == self.mode).unwrap_or(0);
        let tabs = ThemeMode::ALL
            .iter()
            .fold(Tabs::new("settings-theme", selected), |t, m| t.tab(m.label(), None))
            .on_select(move |ix, window, cx| {
                ws.update(cx, |this, cx| this.set_theme_mode(ThemeMode::ALL[ix], window, cx))
            });
        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(div().font_weight(gpui::FontWeight::MEDIUM).child("Theme"))
            .child(div().w(px(300.0)).child(tabs))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme.text_faint)
                    .child("System follows your operating system's light/dark setting."),
            )
    }
}
