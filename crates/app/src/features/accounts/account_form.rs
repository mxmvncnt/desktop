//! Add / edit an account.

use gd_ui::{Button, Tabs, Theme};
use gpui::{App, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};

use super::field_row::FieldRow;
use super::model::Provider;
use super::state::{AccountForm as FormState, AccountInputs};
use super::status_text::StatusText;
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct AccountForm {
    form: FormState,
    inputs: AccountInputs,
    ws: Entity<Workspace>,
}

impl AccountForm {
    pub fn new(form: FormState, inputs: AccountInputs, ws: Entity<Workspace>) -> Self {
        Self { form, inputs, ws }
    }

    fn provider_tabs(&self) -> impl IntoElement + use<> {
        let ws = self.ws.clone();
        let selected = Provider::ALL.iter().position(|p| *p == self.form.provider).unwrap_or(0);
        let tabs = Provider::ALL
            .iter()
            .fold(Tabs::new("account-provider", selected), |t, p| t.tab(p.label(), None))
            .on_select(move |ix, _, cx| ws.update(cx, |this, cx| this.set_account_provider(Provider::ALL[ix], cx)));
        div().w(px(420.0)).child(tabs)
    }

    fn host_field(&self) -> FieldRow {
        let hint = match self.form.provider {
            Provider::GitHubEnterprise => "github.example.com",
            Provider::Generic => "git.example.com",
            _ => "",
        };
        FieldRow::new("Host", "account-host", &self.inputs.host, hint)
    }

    fn buttons(&self) -> impl IntoElement + use<> {
        let (ws_save, ws_cancel, ws_remove) = (self.ws.clone(), self.ws.clone(), self.ws.clone());
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                Button::new("account-save")
                    .primary()
                    .compact()
                    .label(if self.form.editing.is_some() { "Save" } else { "Add account" })
                    .on_click(move |_, _, cx| ws_save.update(cx, |this, cx| this.save_account(cx))),
            )
            .child(
                Button::new("account-cancel")
                    .ghost()
                    .compact()
                    .label("Cancel")
                    .on_click(move |_, _, cx| ws_cancel.update(cx, |this, cx| this.close_account_form(cx))),
            )
            .children(self.form.editing.is_some().then(|| {
                Button::new("account-remove")
                    .style(gd_ui::ButtonStyle::Danger)
                    .compact()
                    .label("Remove")
                    .on_click(move |_, _, cx| ws_remove.update(cx, |this, cx| this.remove_account(cx)))
            }))
    }
}

impl RenderOnce for AccountForm {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let editing = self.form.editing.is_some();
        let inp = &self.inputs;
        let token_hint = if self.form.has_token { "Leave empty to keep the saved token" } else { "Personal access token" };
        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(div().font_weight(gpui::FontWeight::MEDIUM).child(if editing { "Edit account" } else { "Add account" }))
            .child(self.provider_tabs())
            .child(FieldRow::new("Display name", "account-name", &inp.display_name, "Work"))
            .child(self.host_field())
            .child(FieldRow::new("Username", "account-username", &inp.username, "octocat"))
            .child(FieldRow::new("Personal access token", "account-token", &inp.token, token_hint))
            .child(div().text_size(px(12.0)).text_color(theme.text_faint).child("The token is shown while typing and is not validated online."))
            .children(self.form.error.as_deref().map(StatusText::error))
            .child(self.buttons())
    }
}
