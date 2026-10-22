//! The rename / create-from / delete branch dialogs.

use gd_git::{Branch, BranchKind};
use gd_ui::{CheckState, Checkbox, Dialog, Theme};
use gpui::{
    App, Context, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    div, px,
};
use gpui_elements::editable_text::{EditableTextState, actions::{Enter, Escape}};

use super::state::BranchDialog;
use crate::workspace::{Workspace, text_field};

#[derive(IntoElement)]
pub struct BranchDialogView {
    dialog: BranchDialog,
    valid: bool,
    /// Sanitized text of the name input.
    typed: String,
    /// Checked-out branch.
    current: String,
    input: Entity<EditableTextState>,
    ws: Entity<Workspace>,
}

impl BranchDialogView {
    /// `None` when no branch dialog is open.
    pub fn open(ws: &Workspace, cx: &mut Context<Workspace>) -> Option<Self> {
        let current = ws.repo.as_ref()?.read(cx).branch_name();
        Some(Self {
            valid: ws.branch_dialog_valid(cx),
            typed: ws.typed_branch_name(cx),
            dialog: ws.branch_ui.dialog.clone()?,
            current,
            input: ws.branch_ui.input.clone(),
            ws: cx.entity(),
        })
    }
}

impl RenderOnce for BranchDialogView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let dialog = match &self.dialog {
            BranchDialog::Rename { old } => self.rename(old, cx),
            BranchDialog::CreateFrom { base } => self.create_from(base, cx),
            BranchDialog::Delete { branch, remote, also_remote } => self.delete(branch, remote.as_deref(), *also_remote, cx),
        };
        let (ws_ok, ws_cancel) = (self.ws.clone(), self.ws.clone());
        let dialog = dialog
            .confirm_disabled(!self.valid)
            .on_confirm(move |w, cx| ws_ok.update(cx, |this, cx| this.branch_dialog_confirm(w, cx)))
            .on_cancel(move |w, cx| {
                ws_cancel.update(cx, |this, cx| {
                    this.close_branch_ui(w, cx);
                })
            });
        key_capture(&self.ws).child(dialog)
    }
}

impl BranchDialogView {
    fn rename(&self, old: &str, cx: &App) -> Dialog {
        let detail = if self.typed.is_empty() || self.typed == old {
            SharedString::from("Spaces become dashes.")
        } else {
            format!("The branch will be renamed to {}.", self.typed).into()
        };
        Dialog::new("rename-branch", format!("Rename {old}"))
            .body("Choose a new name for this branch.")
            .content(self.name_field("rename-input", cx))
            .detail(detail)
            .confirm_label("Rename branch")
    }

    fn create_from(&self, base: &str, cx: &App) -> Dialog {
        Dialog::new("create-from", format!("Create branch from {base}"))
            .body(format!("The new branch starts at {base} and is checked out."))
            .content(self.name_field("create-from-input", cx))
            .detail(format!("Currently on {}. Uncommitted changes come along.", self.current))
            .confirm_label("Create branch")
    }

    fn delete(&self, branch: &Branch, upstream: Option<&str>, also_remote: bool, cx: &App) -> Dialog {
        let is_remote = branch.kind == BranchKind::Remote;
        let title = if is_remote { "Delete remote branch" } else { "Delete branch" };
        let body = if is_remote {
            format!("Delete {} from the remote? Other people lose it too.", branch.name)
        } else {
            format!("Are you sure you want to delete the branch {}?", branch.name)
        };
        let dlg = Dialog::new("delete-branch", title)
            .body(body)
            .detail("This action cannot be undone.")
            .danger()
            .confirm_label(title);
        match (upstream, is_remote) {
            (Some(remote), false) => dlg.content(self.also_remote_row(remote, also_remote, cx)),
            _ => dlg,
        }
    }

    fn name_field(&self, id: &'static str, cx: &App) -> impl IntoElement + use<> {
        div().mt(px(4.0)).child(text_field(id, &self.input, "Branch name", cx))
    }

    fn also_remote_row(&self, remote: &str, checked: bool, cx: &App) -> impl IntoElement + use<> {
        let theme = Theme::of(cx);
        let ws = self.ws.clone();
        let state = if checked { CheckState::Checked } else { CheckState::Unchecked };
        div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .mt(px(4.0))
            .text_size(px(13.0))
            .text_color(theme.text)
            .child(
                Checkbox::new("delete-remote", state)
                    .on_click(move |_, _, cx| ws.update(cx, |this, cx| this.toggle_also_remote(cx))),
            )
            .child(format!("Also delete the remote branch {remote}"))
    }
}

/// Full-screen wrapper capturing Enter / Esc before the input sees them.
fn key_capture(ws: &Entity<Workspace>) -> gpui::Div {
    let (w1, w2) = (ws.clone(), ws.clone());
    div()
        .absolute()
        .inset_0()
        .capture_action(move |_: &Enter, w, cx| {
            cx.stop_propagation();
            w1.update(cx, |this, cx| this.branch_dialog_confirm(w, cx))
        })
        .capture_action(move |_: &Escape, w, cx| {
            cx.stop_propagation();
            w2.update(cx, |this, cx| {
                this.close_branch_ui(w, cx);
            })
        })
}
