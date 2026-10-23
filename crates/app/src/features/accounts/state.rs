//! Accounts settings state: the account list, the open add/edit form, the
//! git identity as last read, and the text inputs of both.

use gd_git::{EffectiveIdentity, Identity};
use gpui::{App, AppContext, Entity, Task};
use gpui_elements::editable_text::{EditableTextState, StringStorage};

use super::model::{self, Account, Provider};

/// A one-line result shown under a section.
#[derive(Clone, Debug)]
pub struct StatusLine {
    pub text: String,
    pub error: bool,
}

impl StatusLine {
    pub fn ok(text: impl Into<String>) -> Self {
        Self { text: text.into(), error: false }
    }
    pub fn err(text: impl Into<String>) -> Self {
        Self { text: text.into(), error: true }
    }
}

/// The add/edit account form (its text lives in [`AccountInputs`]).
#[derive(Clone, Debug)]
pub struct AccountForm {
    /// `None` = adding a new account.
    pub editing: Option<String>,
    pub provider: Provider,
    /// The stored account already has a token (empty token input keeps it).
    pub has_token: bool,
    pub error: Option<String>,
}

#[derive(Default, Clone)]
pub struct IdentityState {
    pub global: Identity,
    pub local: Identity,
    pub effective: Option<EffectiveIdentity>,
    pub message: Option<StatusLine>,
}

#[derive(Clone)]
pub struct AccountInputs {
    pub global_name: Entity<EditableTextState>,
    pub global_email: Entity<EditableTextState>,
    pub local_name: Entity<EditableTextState>,
    pub local_email: Entity<EditableTextState>,
    pub display_name: Entity<EditableTextState>,
    pub host: Entity<EditableTextState>,
    pub username: Entity<EditableTextState>,
    pub token: Entity<EditableTextState>,
}

pub struct AccountsUi {
    pub accounts: Vec<Account>,
    pub form: Option<AccountForm>,
    pub identity: IdentityState,
    pub inputs: AccountInputs,
    pub(super) load_gen: u64,
    pub(super) task: Option<Task<()>>,
}

impl AccountsUi {
    /// Loads the saved accounts and registers their tokens for git network ops.
    pub fn new(cx: &mut App) -> Self {
        let mk = |cx: &mut App| cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        let accounts = model::load();
        register_credentials(&accounts);
        Self {
            accounts,
            form: None,
            identity: IdentityState::default(),
            inputs: AccountInputs {
                global_name: mk(cx),
                global_email: mk(cx),
                local_name: mk(cx),
                local_email: mk(cx),
                display_name: mk(cx),
                host: mk(cx),
                username: mk(cx),
                token: mk(cx),
            },
            load_gen: 0,
            task: None,
        }
    }
}

/// Push the accounts' tokens to the git layer.
pub fn register_credentials(accounts: &[Account]) {
    gd_git::set_credentials(accounts.iter().filter_map(Account::credential).collect());
}
