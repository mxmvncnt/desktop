//! Settings → Accounts: git identity, then provider accounts (or the
//! add/edit form while it is open). Scrolls when taller than the modal.

use gpui::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce, StatefulInteractiveElement, Styled, Window, div, px,
};

use super::account_form::AccountForm;
use super::account_list::AccountList;
use super::identity_section::IdentitySection;
use super::state::AccountsUi;
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct AccountsPage {
    ui: AccountsView,
    repo_name: Option<String>,
    ws: Entity<Workspace>,
}

/// The slice of [`AccountsUi`] the page renders (cheap clones).
struct AccountsView {
    accounts: Vec<super::model::Account>,
    form: Option<super::state::AccountForm>,
    identity: super::state::IdentityState,
    inputs: super::state::AccountInputs,
}

impl AccountsPage {
    pub fn new(ui: &AccountsUi, repo_name: Option<String>, ws: Entity<Workspace>) -> Self {
        let ui = AccountsView {
            accounts: ui.accounts.clone(),
            form: ui.form.clone(),
            identity: ui.identity.clone(),
            inputs: ui.inputs.clone(),
        };
        Self { ui, repo_name, ws }
    }
}

impl RenderOnce for AccountsPage {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ui = self.ui;
        // The form replaces the whole page so it is fully visible.
        let content = match ui.form {
            Some(form) => AccountForm::new(form, ui.inputs, self.ws).into_any_element(),
            None => div()
                .flex()
                .flex_col()
                .gap(px(22.0))
                .child(IdentitySection::new(ui.identity, ui.inputs, self.repo_name, self.ws.clone()))
                .child(div().h(px(1.0)).bg(gd_ui::Theme::of(cx).border))
                .child(AccountList::new(ui.accounts, self.ws))
                .into_any_element(),
        };
        div()
            .id("settings-accounts-scroll")
            .size_full()
            .overflow_y_scroll()
            .child(
                div().pr(px(8.0)).child(content),
            )
    }
}
