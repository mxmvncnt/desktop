//! Workspace-side stash handlers: menu actions, opening/closing/confirming the
//! stash dialogs. Components call these through their `Entity<Workspace>` prop
//! (`ws.update(cx, |this, cx| this.open_stash_dialog(..))`).

use gd_git::{Branch, Stash};
use gpui::{ClipboardItem, Context, Focusable, Pixels, Point, Window};

use super::state::StashDialog;
use crate::repo::RepoModel;
use crate::features::menus::{MenuAction, MenuTarget};
use crate::workspace::Workspace;

impl Workspace {
    /// Handles stash-related menu actions; returns false for other actions.
    pub fn run_stash_action(
        &mut self,
        action: MenuAction,
        target: &MenuTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let stash = match target {
            MenuTarget::Stash(s) => Some(s.clone()),
            _ => None,
        };
        match (action, stash) {
            (MenuAction::StashAll, _) => self.open_stash_create(false, window, cx),
            (MenuAction::StashSelected, _) => self.open_stash_create(true, window, cx),
            (MenuAction::StashApply, Some(s)) => self.stash_op(cx, move |r, cx| r.stash_apply(s.sha, cx)),
            (MenuAction::StashPop, Some(s)) => self.stash_op(cx, move |r, cx| r.stash_pop(s.sha, cx)),
            (MenuAction::StashDrop, Some(s)) => self.open_stash_dialog(StashDialog::Drop(s), window, cx),
            (MenuAction::StashRename, Some(s)) => {
                self.set_stash_input(&s.message, cx);
                self.open_stash_dialog(StashDialog::Rename(s), window, cx)
            }
            (MenuAction::StashCopy, Some(s)) => cx.write_to_clipboard(ClipboardItem::new_string(s.title())),
            _ => return false,
        }
        true
    }

    pub fn open_stash_menu(&mut self, stash: Stash, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        self.open_menu(position, MenuTarget::Stash(stash), window, cx);
    }

    /// Ctrl+Shift+S / the header menu. `selected_only` stashes the checked files.
    pub fn open_stash_create(&mut self, selected_only: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else { return };
        let r = repo.read(cx);
        if r.changes().is_empty() {
            return;
        }
        if selected_only && !r.has_included() {
            repo.update(cx, |r, cx| {
                r.notice = Some("Check at least one file to stash".into());
                cx.notify()
            });
            return;
        }
        self.set_stash_input("", cx);
        self.open_stash_dialog(StashDialog::Create { selected_only }, window, cx);
    }

    /// Called when a checkout was requested with a dirty working tree.
    pub fn open_switch_dialog(&mut self, branch: Branch, cx: &mut Context<Self>) {
        self.menu = None;
        self.popover = None;
        self.dialog = None;
        // Desktop's default: leave the changes behind (stash).
        self.stash_ui.dialog = Some(StashDialog::Switch { branch, stash: true });
        cx.notify();
    }

    /// Swap the open dialog's content (option rows toggling a choice).
    pub(super) fn update_stash_dialog(&mut self, d: StashDialog, cx: &mut Context<Self>) {
        self.stash_ui.dialog = Some(d);
        cx.notify();
    }

    pub fn close_stash_dialog(&mut self, cx: &mut Context<Self>) {
        if self.stash_ui.dialog.take().is_some() {
            cx.notify();
        }
    }

    pub fn stash_confirm(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.stash_ui.dialog.take() else { return };
        cx.notify();
        let Some(repo) = self.repo.clone() else { return };
        let text = self.stash_ui.input.read(cx).as_str().trim().to_string();
        match d {
            StashDialog::Create { selected_only } => {
                let paths = if selected_only { stash_paths(repo.read(cx)) } else { Vec::new() };
                repo.update(cx, |r, cx| r.stash_push(text, paths, cx));
            }
            StashDialog::Rename(s) => repo.update(cx, |r, cx| r.stash_rename(s, text, cx)),
            StashDialog::Drop(s) => repo.update(cx, |r, cx| r.stash_drop(s.sha, cx)),
            StashDialog::Switch { branch, stash } => {
                repo.update(cx, |r, cx| r.checkout_with_stash(branch, stash, cx))
            }
        }
    }

    pub(super) fn open_stash_dialog(&mut self, d: StashDialog, window: &mut Window, cx: &mut Context<Self>) {
        let wants_input = matches!(d, StashDialog::Create { .. } | StashDialog::Rename(_));
        self.menu = None;
        self.popover = None;
        self.dialog = None;
        self.stash_ui.dialog = Some(d);
        if wants_input {
            window.focus(&self.stash_ui.input.focus_handle(cx), cx);
        } else {
            self.focus(window, cx);
        }
        cx.notify();
    }

    fn stash_op(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut RepoModel, &mut Context<RepoModel>)) {
        if let Some(repo) = &self.repo {
            repo.update(cx, f);
        }
    }

    fn set_stash_input(&mut self, text: &str, cx: &mut Context<Self>) {
        self.stash_ui.input.update(cx, |s, cx| s.emplace(text, cx));
    }
}

/// Pathspecs for "stash the checked files": include the old path of renames
/// so both halves of the rename are stashed.
fn stash_paths(r: &RepoModel) -> Vec<String> {
    let mut v = Vec::new();
    for c in r.changes() {
        if r.excluded.contains(&c.path) {
            continue;
        }
        v.push(c.path.clone());
        if let Some(old) = &c.old_path {
            v.push(old.clone());
        }
    }
    v
}
