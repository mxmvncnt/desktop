//! Confirm dialog for discarding one file or all changes.

use gd_ui::Dialog;
use gpui::{App, Context, Entity, IntoElement, RenderOnce, Window};

use super::state::ConfirmDialog;
use crate::shared::plural::plural;
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct DiscardDialogView {
    title: String,
    body: String,
    confirm_label: &'static str,
    ws: Entity<Workspace>,
}

impl DiscardDialogView {
    /// `None` when no confirm dialog is open.
    pub fn open(ws: &Workspace, cx: &mut Context<Workspace>) -> Option<Self> {
        let (title, body, confirm_label) = match ws.dialog.as_ref()? {
            ConfirmDialog::DiscardFile(c) => (
                "Confirm discard changes".to_string(),
                format!("Are you sure you want to discard all changes to {}?", c.path),
                "Discard changes",
            ),
            ConfirmDialog::DiscardAll(all) => (
                "Confirm discard all changes".to_string(),
                format!(
                    "Are you sure you want to discard all changes to {} file{}?",
                    all.len(),
                    plural(all.len() as u32)
                ),
                "Discard all changes",
            ),
        };
        Some(Self { title, body, confirm_label, ws: cx.entity() })
    }
}

impl RenderOnce for DiscardDialogView {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (ok, cancel) = (self.ws.clone(), self.ws);
        Dialog::new("discard-dialog", self.title)
            .body(self.body)
            .detail("This can't be undone. Untracked files are deleted permanently.")
            .confirm_label(self.confirm_label)
            .danger()
            .on_confirm(move |_, cx| ok.update(cx, |this, cx| this.confirm_dialog(cx)))
            .on_cancel(move |_, cx| cancel.update(cx, |this, cx| this.close_dialog(cx)))
    }
}
