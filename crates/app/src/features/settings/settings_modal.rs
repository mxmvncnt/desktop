//! Settings modal: a sidebar of categories and the selected category's page.
//! Esc, the close button, a backdrop click, or the toolbar gear closes it.

use gd_ui::{Icon, divider, icon_button, motion, overlay_card, theme::scrim};
use gpui::{
    AnyElement, App, Context, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement, RenderOnce,
    Styled, Window, deferred, div, px,
};

use super::appearance_page::AppearancePage;
use super::diff_page::{DiffOptionValues, DiffPage};
use super::settings_nav::SettingsNav;
use super::{SettingsPage, ThemeMode};
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct SettingsModal {
    page: SettingsPage,
    theme: ThemeMode,
    diff: DiffOptionValues,
    ws: Entity<Workspace>,
}

impl SettingsModal {
    /// `None` while the modal is closed.
    pub fn open(ws: &Workspace, cx: &mut Context<Workspace>) -> Option<Self> {
        let s = &ws.settings;
        ws.settings_open.then(|| Self {
            page: ws.settings_page,
            theme: s.theme,
            diff: DiffOptionValues {
                split: s.diff_split,
                wrap: s.diff_wrap,
                hide_whitespace: s.diff_ignore_whitespace,
            },
            ws: cx.entity(),
        })
    }

    fn body(&self) -> AnyElement {
        match self.page {
            SettingsPage::Appearance => AppearancePage::new(self.theme, self.ws.clone()).into_any_element(),
            SettingsPage::Diff => DiffPage::new(self.diff, self.ws.clone()).into_any_element(),
        }
    }

    fn title_bar(&self) -> impl IntoElement {
        let ws = self.ws.clone();
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .h(px(52.0))
            .pl(px(24.0))
            .pr(px(12.0))
            .child(div().text_size(px(15.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(self.page.label()))
            .child(
                icon_button("settings-close", Icon::Close)
                    .compact()
                    .on_click(move |_, _, cx| close(&ws, cx)),
            )
    }

    fn card(&self, cx: &mut App) -> gpui::Stateful<gpui::Div> {
        overlay_card(cx)
            .id("settings-card")
            .occlude()
            .w(px(700.0))
            .h(px(440.0))
            .flex()
            .child(SettingsNav::new(self.page, self.ws.clone()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(self.title_bar())
                    .child(divider(cx))
                    .child(div().flex_1().min_h_0().p(px(24.0)).child(self.body())),
            )
    }
}

impl RenderOnce for SettingsModal {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ws = self.ws.clone();
        let backdrop = div()
            .id("settings-backdrop")
            .absolute()
            .inset_0()
            .bg(scrim(0.5))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| close(&ws, cx));
        deferred(
            div()
                .absolute()
                .inset_0()
                .child(motion::fade_quick("settings-scrim-in", backdrop))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(motion::dialog_in("settings-in", self.card(cx))),
                ),
        )
    }
}

fn close(ws: &Entity<Workspace>, cx: &mut App) {
    ws.update(cx, |this, cx| this.toggle_settings(cx))
}
