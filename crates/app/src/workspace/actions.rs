//! Keyboard / menu action routing for the root view.

use gpui::{Context, InteractiveElement, Stateful, Div, Window};

use super::state::{Popover, Workspace};
use crate::actions::*;
use crate::features::merge::MergeMode;
use crate::repo::{RemoteOp, Tab};

/// Register every workspace-level action handler on the root element.
pub(super) fn bind(root: Stateful<Div>, cx: &mut Context<Workspace>) -> Stateful<Div> {
    bind_menu_keys(bind_commands(bind_popovers(root, cx), cx), cx)
}

fn bind_popovers(root: Stateful<Div>, cx: &mut Context<Workspace>) -> Stateful<Div> {
    root.on_action(cx.listener(|this, _: &ToggleRepositoryList, w, cx| {
        this.toggle_popover(Popover::Repositories, w, cx)
    }))
    .on_action(cx.listener(|this, _: &ToggleBranchList, w, cx| this.toggle_popover(Popover::Branches, w, cx)))
    .on_action(cx.listener(|this, _: &NewBranch, w, cx| this.toggle_popover(Popover::NewBranch, w, cx)))
    .on_action(cx.listener(|this, _: &AddRepository, _, cx| this.prompt_add_repository(cx)))
}

fn bind_commands(root: Stateful<Div>, cx: &mut Context<Workspace>) -> Stateful<Div> {
    root.on_action(cx.listener(|this, _: &ShowChanges, _, cx| this.set_tab(Tab::Changes, cx)))
        .on_action(cx.listener(|this, _: &ShowHistory, _, cx| this.set_tab(Tab::History, cx)))
        .on_action(cx.listener(|this, _: &MergeIntoCurrent, w, cx| {
            this.open_merge_dialog(MergeMode::Merge, None, w, cx)
        }))
        .on_action(cx.listener(|this, _: &Fetch, _, cx| this.remote_action(RemoteOp::Fetch, cx)))
        .on_action(cx.listener(|this, _: &Pull, _, cx| this.remote_action(RemoteOp::Pull, cx)))
        .on_action(cx.listener(|this, _: &Push, _, cx| this.remote_action(RemoteOp::Push, cx)))
        .on_action(cx.listener(|this, _: &OpenSettings, _, cx| this.toggle_settings(cx)))
        .on_action(cx.listener(|this, _: &Refresh, _, cx| this.refresh(cx)))
        .on_action(cx.listener(|this, _: &Commit, w, cx| this.commit(w, cx)))
        .on_action(cx.listener(|this, _: &StashChanges, w, cx| this.open_stash_create(false, w, cx)))
}

fn bind_menu_keys(root: Stateful<Div>, cx: &mut Context<Workspace>) -> Stateful<Div> {
    root.on_action(cx.listener(|this, _: &MenuNext, _, cx| this.step_open_menu(1, cx)))
        .on_action(cx.listener(|this, _: &MenuPrevious, _, cx| this.step_open_menu(-1, cx)))
        .on_action(cx.listener(|this, _: &MenuConfirm, w, cx| {
            if !this.hist_confirm_key(w, cx) {
                this.menu_confirm(w, cx)
            }
        }))
        .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.select_step(1, cx)))
        .on_action(cx.listener(|this, _: &SelectPrevious, _, cx| this.select_step(-1, cx)))
        .on_action(cx.listener(|this, _: &Dismiss, w, cx| this.dismiss_top_overlay(w, cx)))
}

impl Workspace {
    /// Any menu / dialog / modal that wants Esc, Enter and arrows.
    pub(super) fn overlay_open(&self) -> bool {
        self.history_ui.is_open()
            || self.menu.is_some()
            || self.dialog.is_some()
            || self.settings_open
            || self.diff_view.menu.is_some()
            || self.stash_ui.dialog.is_some()
            || self.merge_ui.wants_menu_context()
            || self.branch_ui.wants_menu_context()
    }

    fn step_open_menu(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.history_ui.menu.is_some() {
            self.hist_menu_step(delta, cx)
        } else {
            self.menu_step(delta, cx)
        }
    }

    /// Esc: close the topmost overlay, in z-order.
    fn dismiss_top_overlay(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.settings_open {
            self.toggle_settings(cx)
        } else if self.hist_dismiss(w, cx) {
        } else if self.stash_ui.dialog.is_some() {
            self.close_stash_dialog(cx)
        } else if self.close_merge_ui(w, cx) || self.close_branch_ui(w, cx) {
        } else if self.dialog.is_some() {
            self.close_dialog(cx)
        } else if self.diff_view.menu.is_some() {
            self.close_diff_options(cx)
        } else if self.menu.is_some() {
            self.close_menu(cx)
        } else if self.popover.is_some() {
            self.close_popover(w, cx)
        }
    }
}
