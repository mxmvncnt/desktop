//! `impl Workspace` handlers for the Accounts settings page: git identity
//! load/save (background `git config`) and account add/edit/remove.

use gd_git::{Identity, Repository};
use gpui::{AppContext as _, Context, Entity};
use gpui_elements::editable_text::EditableTextState;

use super::model::{self, Account, Provider, normalize_host};
use super::state::{AccountForm, StatusLine, register_credentials};
use crate::workspace::Workspace;

impl Workspace {
    fn current_git_repo(&self, cx: &Context<Self>) -> Option<Repository> {
        self.repo.as_ref().map(|r| r.read(cx).repo.clone())
    }

    fn input_text(&self, input: &Entity<EditableTextState>, cx: &Context<Self>) -> String {
        input.read(cx).as_str().trim().to_string()
    }

    fn set_input(input: &Entity<EditableTextState>, text: &str, cx: &mut Context<Self>) {
        input.update(cx, |s, cx| s.emplace(text, cx));
    }

    /// Read the global (and, with a repo open, local + effective) identity off the UI thread.
    pub fn load_identity(&mut self, cx: &mut Context<Self>) {
        let repo = self.current_git_repo(cx);
        self.accounts_ui.load_gen += 1;
        let gen_ = self.accounts_ui.load_gen;
        self.accounts_ui.task = Some(cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move {
                    let global = gd_git::global_identity()?;
                    let repo_part = match repo {
                        Some(r) => Some((r.local_identity()?, r.effective_identity()?)),
                        None => None,
                    };
                    Ok::<_, gd_git::GitError>((global, repo_part))
                })
                .await;
            this.update(cx, |this, cx| {
                if gen_ != this.accounts_ui.load_gen {
                    return;
                }
                this.apply_identity(res, cx);
            })
            .ok();
        }));
    }

    fn apply_identity(
        &mut self,
        res: Result<(Identity, Option<(Identity, gd_git::EffectiveIdentity)>), gd_git::GitError>,
        cx: &mut Context<Self>,
    ) {
        match res {
            Ok((global, repo_part)) => {
                let id = &mut self.accounts_ui.identity;
                id.global = global;
                (id.local, id.effective) = match repo_part {
                    Some((l, e)) => (l, Some(e)),
                    None => (Identity::default(), None),
                };
                let (g, l) = (id.global.clone(), id.local.clone());
                let inp = &self.accounts_ui.inputs;
                for (input, text) in [
                    (&inp.global_name, g.name),
                    (&inp.global_email, g.email),
                    (&inp.local_name, l.name),
                    (&inp.local_email, l.email),
                ] {
                    Self::set_input(input, &text, cx);
                }
            }
            Err(e) => self.accounts_ui.identity.message = Some(StatusLine::err(e.to_string())),
        }
        cx.notify();
    }

    pub(super) fn save_global_identity(&mut self, cx: &mut Context<Self>) {
        let inp = &self.accounts_ui.inputs;
        let id = Identity { name: self.input_text(&inp.global_name, cx), email: self.input_text(&inp.global_email, cx) };
        self.write_identity(None, id, "Saved to your global git config", cx);
    }

    pub(super) fn save_local_identity(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.current_git_repo(cx) else { return };
        let inp = &self.accounts_ui.inputs;
        let id = Identity { name: self.input_text(&inp.local_name, cx), email: self.input_text(&inp.local_email, cx) };
        self.write_identity(Some(repo), id, "Saved to this repository's config", cx);
    }

    pub(super) fn clear_local_identity(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.current_git_repo(cx) else { return };
        self.write_identity(Some(repo), Identity::default(), "Override removed", cx);
    }

    /// Write `id` to the repository (or the global config when `repo` is `None`), then reload.
    fn write_identity(&mut self, repo: Option<Repository>, id: Identity, done: &'static str, cx: &mut Context<Self>) {
        self.accounts_ui.identity.message = None;
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move {
                    match repo {
                        Some(r) => r.set_local_identity(&id),
                        None => gd_git::set_global_identity(&id),
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                this.load_identity(cx);
                this.accounts_ui.identity.message = Some(match res {
                    Ok(()) => StatusLine::ok(done),
                    Err(e) => StatusLine::err(e.to_string()),
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    // ---- accounts ----------------------------------------------------------

    pub(super) fn open_account_form(&mut self, editing: Option<String>, cx: &mut Context<Self>) {
        let acct = editing.as_ref().and_then(|id| self.accounts_ui.accounts.iter().find(|a| &a.id == id)).cloned();
        let provider = acct.as_ref().map_or(Provider::GitHub, |a| a.provider);
        let a = acct.clone().unwrap_or(Account {
            id: String::new(),
            provider,
            name: String::new(),
            host: provider.fixed_host().unwrap_or_default().into(),
            username: String::new(),
            token: String::new(),
        });
        let inp = &self.accounts_ui.inputs;
        for (input, text) in [(&inp.display_name, &a.name), (&inp.host, &a.host), (&inp.username, &a.username), (&inp.token, &String::new())] {
            Self::set_input(input, text, cx);
        }
        self.accounts_ui.form =
            Some(AccountForm { editing, provider, has_token: acct.is_some_and(|a| !a.token.is_empty()), error: None });
        cx.notify();
    }

    pub(super) fn set_account_provider(&mut self, provider: Provider, cx: &mut Context<Self>) {
        let host_input = self.accounts_ui.inputs.host.clone();
        let Some(form) = &mut self.accounts_ui.form else { return };
        form.provider = provider;
        // Hosted providers have a fixed host; custom ones keep what was typed
        // unless it was a fixed host of the previous choice.
        let typed = self.input_text(&host_input, cx);
        let was_fixed = Provider::ALL.iter().any(|p| p.fixed_host() == Some(typed.as_str()));
        match provider.fixed_host() {
            Some(h) => Self::set_input(&host_input, h, cx),
            None if was_fixed => Self::set_input(&host_input, "", cx),
            None => {}
        }
        cx.notify();
    }

    pub(super) fn close_account_form(&mut self, cx: &mut Context<Self>) {
        self.accounts_ui.form = None;
        cx.notify();
    }

    pub(super) fn save_account(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.accounts_ui.form.clone() else { return };
        let inp = &self.accounts_ui.inputs;
        let host = normalize_host(&self.input_text(&inp.host, cx));
        let username = self.input_text(&inp.username, cx);
        let typed_token = self.input_text(&inp.token, cx);
        let mut name = self.input_text(&inp.display_name, cx);
        if host.is_empty() {
            return self.account_form_error("Enter a host, e.g. github.com", cx);
        }
        if name.is_empty() {
            name = if username.is_empty() { host.clone() } else { format!("{username}@{host}") };
        }
        let list = &mut self.accounts_ui.accounts;
        let existing = form.editing.as_ref().and_then(|id| list.iter_mut().find(|a| &a.id == id));
        match existing {
            Some(a) => {
                a.provider = form.provider;
                (a.name, a.host, a.username) = (name, host, username);
                if !typed_token.is_empty() {
                    a.token = typed_token;
                }
            }
            None => list.push(Account { id: model::new_id(), provider: form.provider, name, host, username, token: typed_token }),
        }
        self.persist_accounts(cx);
        self.accounts_ui.form = None;
        cx.notify();
    }

    pub(super) fn remove_account(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.accounts_ui.form.as_ref().and_then(|f| f.editing.clone()) else { return };
        self.accounts_ui.accounts.retain(|a| a.id != id);
        self.persist_accounts(cx);
        self.accounts_ui.form = None;
        cx.notify();
    }

    fn account_form_error(&mut self, msg: &str, cx: &mut Context<Self>) {
        if let Some(f) = &mut self.accounts_ui.form {
            f.error = Some(msg.into());
        }
        cx.notify();
    }

    /// Save to disk (small files) and refresh the git credential registry.
    fn persist_accounts(&mut self, _cx: &mut Context<Self>) {
        model::save(&self.accounts_ui.accounts);
        register_credentials(&self.accounts_ui.accounts);
    }
}
