//! "Accounts" section: the list and the Add button.

use gd_ui::{Button, Icon, Theme};
use gpui::{App, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};

use super::account_row::AccountRow;
use super::model::Account;
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct AccountList {
    accounts: Vec<Account>,
    ws: Entity<Workspace>,
}

impl AccountList {
    pub fn new(accounts: Vec<Account>, ws: Entity<Workspace>) -> Self {
        Self { accounts, ws }
    }

    fn header(&self) -> impl IntoElement + use<> {
        let ws = self.ws.clone();
        div()
            .flex()
            .items_center()
            .justify_between()
            .child(div().font_weight(gpui::FontWeight::MEDIUM).child("Accounts"))
            .child(
                Button::new("account-add")
                    .compact()
                    .icon(Icon::Plus)
                    .label("Add account")
                    .on_click(move |_, _, cx| ws.update(cx, |this, cx| this.open_account_form(None, cx))),
            )
    }
}

impl RenderOnce for AccountList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let caption = |t: &'static str| div().text_size(px(12.0)).text_color(theme.text_faint).child(t);
        let ws = self.ws.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(self.header())
            .child(caption("Tokens are used for HTTPS fetch, pull and push to the account's host. They are stored in a private (0600) file in the config folder."))
            .children(self.accounts.iter().cloned().map(|a| AccountRow::new(a, ws.clone())))
            .children(self.accounts.is_empty().then(|| caption("No accounts yet.")))
    }
}
