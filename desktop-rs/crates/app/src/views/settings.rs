//! Settings modal: a sidebar of categories and the selected category's page.
//! Shown while `Workspace::settings_open`. Esc, the close button, a backdrop
//! click, or the toolbar gear closes it.

use gd_ui::{
    Icon, Tabs, Theme, divider, icon, icon_button, motion, overlay_card, pressable,
    theme::{ink, scrim},
};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled,
    deferred, div, px,
};

use super::diff_options::option_row;
use crate::settings::ThemeMode;
use crate::workspace::Workspace;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SettingsPage {
    #[default]
    Appearance,
    Diff,
}

impl SettingsPage {
    const ALL: [SettingsPage; 2] = [Self::Appearance, Self::Diff];

    fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Diff => "Diff",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Self::Appearance => Icon::Eye,
            Self::Diff => Icon::Tuning,
        }
    }
}

impl Workspace {
    pub fn toggle_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_open = !self.settings_open;
        cx.notify();
    }

    pub fn render_settings(&mut self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        if !self.settings_open {
            return None;
        }
        let theme = Theme::of(cx).clone();
        let ws = cx.entity();
        let page = self.settings_page;

        let nav = SettingsPage::ALL.into_iter().fold(
            div().flex().flex_col().gap(px(2.0)).p(px(8.0)).w(px(188.0)).flex_none()
                .border_r_1().border_color(theme.border).bg(ink(0.02)),
            |nav, p| {
                let ws = ws.clone();
                nav.child(
                    pressable(p.label(), p == page, cx)
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .h(px(30.0))
                        .px(px(10.0))
                        .rounded(px(gd_ui::theme::CONTROL_RADIUS))
                        .text_size(px(13.0))
                        .text_color(if p == page { theme.text } else { theme.text_muted })
                        .child(icon(p.icon()).size(px(15.0)).text_color(if p == page { theme.text } else { theme.text_faint }))
                        .child(p.label())
                        .on_click(move |_, _, cx| {
                            ws.update(cx, |this, cx| {
                                this.settings_page = p;
                                cx.notify()
                            })
                        }),
                )
            },
        );

        let body: AnyElement = match page {
            SettingsPage::Appearance => self.appearance_page(cx),
            SettingsPage::Diff => self.diff_page(cx),
        };

        let close = {
            let ws = ws.clone();
            move |cx: &mut gpui::App| ws.update(cx, |this, cx| this.toggle_settings(cx))
        };
        let close_btn = {
            let close = close.clone();
            icon_button("settings-close", Icon::Close).compact().on_click(move |_, _, cx| close(cx))
        };
        let card = overlay_card(cx)
            .id("settings-card")
            .occlude()
            .w(px(700.0))
            .h(px(440.0))
            .flex()
            .child(nav)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_between()
                            .h(px(52.0))
                            .pl(px(24.0))
                            .pr(px(12.0))
                            .child(
                                div()
                                    .text_size(px(15.0))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(page.label()),
                            )
                            .child(close_btn),
                    )
                    .child(divider(cx))
                    .child(div().flex_1().min_h_0().p(px(24.0)).child(body)),
            );

        let backdrop = div()
            .id("settings-backdrop")
            .absolute()
            .inset_0()
            .bg(scrim(0.5))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| close(cx));
        Some(deferred(
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
                        .child(motion::dialog_in("settings-in", card)),
                ),
        ))
    }

    fn appearance_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::of(cx).clone();
        let ws = cx.entity();
        let selected = ThemeMode::ALL.iter().position(|m| *m == self.settings.theme).unwrap_or(0);
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
            .into_any_element()
    }

    fn diff_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::of(cx).clone();
        let ws = cx.entity();
        let (split, wrap, hide_ws) =
            (self.settings.diff_split, self.settings.diff_wrap, self.settings.diff_ignore_whitespace);
        let ws_tabs = ws.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(div().font_weight(gpui::FontWeight::MEDIUM).child("Display"))
            .child(
                div().w(px(300.0)).pb(px(6.0)).child(
                    Tabs::new("settings-diff-display", usize::from(split))
                        .tab("Unified", None)
                        .tab("Split", None)
                        .on_select(move |ix, _, cx| {
                            ws_tabs.update(cx, |this, cx| this.set_diff_split(ix == 1, cx))
                        }),
                ),
            )
            .child(option_row("settings-diff-wrap", "Wrap long lines", wrap, ws.clone(), move |this, cx| {
                this.set_diff_wrap(!wrap, cx)
            }, cx))
            .child(option_row("settings-diff-ws", "Hide whitespace changes", hide_ws, ws, move |this, cx| {
                this.set_ignore_whitespace(!hide_ws, cx)
            }, cx))
            .child(
                div()
                    .pt(px(6.0))
                    .text_size(px(12.0))
                    .text_color(theme.text_faint)
                    .child("These apply to every repository."),
            )
            .into_any_element()
    }
}
