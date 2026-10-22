//! Workspace-side menu handlers: open/close the context menu and the discard
//! dialog.

use gd_git::FileStatus;
use gpui::{Context, Pixels, Point, Window};

use super::state::{ConfirmDialog, MenuTarget, OpenMenu};
use crate::workspace::Workspace;

impl Workspace {
    pub fn open_menu(&mut self, position: Point<Pixels>, target: MenuTarget, window: &mut Window, cx: &mut Context<Self>) {
        self.popover = None;
        self.dialog = None;
        self.menu = Some(OpenMenu { position, target, selected: None });
        self.menu_gen += 1;
        // Focus the root so up/down/Enter/Esc reach us.
        self.focus(window, cx);
        cx.notify();
    }

    pub fn close_menu(&mut self, cx: &mut Context<Self>) {
        if self.menu.take().is_some() {
            cx.notify();
        }
    }

    /// Open the menu for list row `ix` (resolved against the live model).
    pub fn open_file_menu(&mut self, ix: usize, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(change) = self.repo.as_ref().and_then(|r| r.read(cx).changes().get(ix).cloned()) else {
            return;
        };
        if change.status == FileStatus::Conflicted {
            return self.open_conflict_menu(change, position, window, cx);
        }
        self.open_menu(position, MenuTarget::File(change), window, cx);
    }

    pub fn open_header_menu(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if self.repo.as_ref().is_some_and(|r| !r.read(cx).changes().is_empty()) {
            self.open_menu(position, MenuTarget::Header, window, cx);
        }
    }

    pub fn close_dialog(&mut self, cx: &mut Context<Self>) {
        if self.dialog.take().is_some() {
            cx.notify();
        }
    }

    pub fn confirm_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.dialog.take() else { return };
        cx.notify();
        let Some(repo) = self.repo.clone() else { return };
        let changes = match dialog {
            ConfirmDialog::DiscardFile(c) => vec![c],
            ConfirmDialog::DiscardAll(all) => all,
        };
        repo.update(cx, |r, cx| r.discard(changes, cx));
    }
}
