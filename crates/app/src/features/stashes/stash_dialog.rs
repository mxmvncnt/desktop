//! The stash dialogs: create / rename / drop and the switch-branch choice.

use gd_git::{Branch, Stash};
use gd_ui::Dialog;
use gpui::{App, Context, Div, Entity, IntoElement, RenderOnce, Window, div, prelude::*, px};
use gpui_elements::editable_text::{EditableTextState, actions::Enter};

use super::option_row::OptionRow;
use super::state::StashDialog;
use crate::repo::RepoModel;
use crate::shared::plural::plural;
use crate::shared::text_field::text_field;
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct StashDialogView {
    dialog: StashDialog,
    ws: Entity<Workspace>,
    repo: Entity<RepoModel>,
    input: Entity<EditableTextState>,
}

impl StashDialogView {
    /// `None` when no stash dialog is open.
    pub fn open(ws: &Workspace, cx: &mut Context<Workspace>) -> Option<Self> {
        Some(Self {
            dialog: ws.stash_ui.dialog.clone()?,
            ws: cx.entity(),
            repo: ws.repo.clone()?,
            input: ws.stash_ui.input.clone(),
        })
    }
}

impl RenderOnce for StashDialogView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let dialog = match &self.dialog {
            StashDialog::Create { selected_only } => self.create(*selected_only, cx),
            StashDialog::Rename(s) => self.rename(s, cx),
            StashDialog::Drop(s) => drop_dialog(s),
            StashDialog::Switch { branch, stash } => self.switch(branch, *stash, cx),
        };
        let (ws, ws2) = (self.ws.clone(), self.ws);
        dialog
            .on_confirm(move |_, cx| ws.update(cx, |this, cx| this.stash_confirm(cx)))
            .on_cancel(move |_, cx| ws2.update(cx, |this, cx| this.close_stash_dialog(cx)))
    }
}

impl StashDialogView {
    fn create(&self, selected_only: bool, cx: &App) -> Dialog {
        let r = self.repo.read(cx);
        let n = r.changes().len();
        // Stashing works on whole files, partially selected ones included.
        let m = r.changes().iter().filter(|c| !r.excluded.contains(&c.path)).count();
        let count = if selected_only { m } else { n };
        let current = r.branch_name();
        let mut content = div().flex().flex_col().gap(px(10.0)).pt(px(4.0));
        if m > 0 && m < n {
            content = content
                .child(self.scope_row("stash-scope-all", false, selected_only, format!("All changes ({n} file{})", plural(n as u32))))
                .child(self.scope_row("stash-scope-selected", true, selected_only, format!("Only the checked files ({m})")));
        }
        content = content.child(self.message_field("Message (optional)", cx));
        Dialog::new("stash-dialog", if selected_only { "Stash selected files" } else { "Stash all changes" })
            .body(format!(
                "Save {count} changed file{} to a new stash and restore a clean working tree.",
                plural(count as u32)
            ))
            .detail(format!("Untracked files are included. You can restore it any time from the Stashes list on {current}."))
            .content(content)
            .confirm_label("Stash changes")
    }

    fn rename(&self, s: &Stash, cx: &App) -> Dialog {
        Dialog::new("stash-dialog", "Rename stash")
            .body(format!("A new message for “{}”.", s.title()))
            .detail("The stash moves to the top of the list.")
            .content(div().pt(px(4.0)).child(self.message_field("Message", cx)))
            .confirm_label("Rename")
    }

    fn switch(&self, branch: &Branch, stash: bool, cx: &App) -> Dialog {
        let current = self.repo.read(cx).branch_name();
        let target = &branch.name;
        let content = div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .pt(px(4.0))
            .child(
                self.switch_row("switch-stash", branch, true, stash, format!("Leave my changes on {current}"))
                    .detail("Your in-progress work will be stashed on this branch so you can come back to it later."),
            )
            .child(
                self.switch_row("switch-bring", branch, false, stash, format!("Bring my changes to {target}"))
                    .detail("Your in-progress work will follow you to the new branch."),
            );
        Dialog::new("switch-branch-dialog", format!("Switch to {target}"))
            .body(format!("You have changes on {current}. What would you like to do with them?"))
            .content(content)
            .confirm_label("Switch branch")
    }

    /// Radio row choosing the scope of a new stash.
    fn scope_row(&self, id: &'static str, selected_only: bool, current: bool, title: String) -> OptionRow {
        let ws = self.ws.clone();
        OptionRow::new(id, selected_only == current, title, move |_, cx| {
            ws.update(cx, |t, cx| t.update_stash_dialog(StashDialog::Create { selected_only }, cx))
        })
    }

    /// Radio row choosing whether to stash (`choice`) or bring the changes along.
    fn switch_row(&self, id: &'static str, branch: &Branch, choice: bool, current: bool, title: String) -> OptionRow {
        let (ws, branch) = (self.ws.clone(), branch.clone());
        OptionRow::new(id, choice == current, title, move |_, cx| {
            let d = StashDialog::Switch { branch: branch.clone(), stash: choice };
            ws.update(cx, |t, cx| t.update_stash_dialog(d, cx))
        })
    }

    fn message_field(&self, placeholder: &str, cx: &App) -> Div {
        let ws = self.ws.clone();
        div()
            .capture_action(move |_: &Enter, _, cx| {
                cx.stop_propagation();
                ws.update(cx, |this, cx| this.stash_confirm(cx))
            })
            .child(text_field("stash-message", &self.input, placeholder, cx))
    }
}

fn drop_dialog(s: &Stash) -> Dialog {
    Dialog::new("stash-dialog", "Discard this stash?")
        .body(format!("Are you sure you want to discard “{}”?", s.title()))
        .detail("This can't be undone. The stashed changes are deleted.")
        .confirm_label("Discard stash")
        .danger()
}
