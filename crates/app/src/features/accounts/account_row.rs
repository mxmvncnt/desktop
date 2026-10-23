//! One configured account in the list.

use gd_ui::{Button, Icon, Theme, icon, theme::CONTROL_RADIUS};
use gpui::{App, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};

use super::model::Account;
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct AccountRow {
    account: Account,
    ws: Entity<Workspace>,
}

impl AccountRow {
    pub fn new(account: Account, ws: Entity<Workspace>) -> Self {
        Self { account, ws }
    }
}

impl RenderOnce for AccountRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let a = self.account;
        let id = a.id.clone();
        let ws = self.ws;
        let token = if a.token.is_empty() { "no token" } else { "token saved" };
        let detail = if a.username.is_empty() {
            format!("{} · {} · {token}", a.provider.full_label(), a.host)
        } else {
            format!("{} · {}@{} · {token}", a.provider.full_label(), a.username, a.host)
        };
        div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .px(px(10.0))
            .py(px(8.0))
            .rounded(px(CONTROL_RADIUS))
            .border_1()
            .border_color(theme.border)
            .child(icon(Icon::Globe).size(px(16.0)).text_color(theme.text_faint))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_size(px(13.0)).child(a.name))
                    .child(div().text_size(px(12.0)).text_color(theme.text_faint).child(detail)),
            )
            .child(
                Button::new(format!("account-edit-{id}"))
                    .ghost()
                    .compact()
                    .icon(Icon::Pen)
                    .label("Edit")
                    .on_click(move |_, _, cx| ws.update(cx, |this, cx| this.open_account_form(Some(id.clone()), cx))),
            )
    }
}
