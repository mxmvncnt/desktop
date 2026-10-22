//! Workspace-side picker handlers: build the popover components from the
//! workspace state, keyboard cursor, and the actions the popovers trigger.

use gpui::{Context, Window};

use super::branch_picker::BranchPicker;
use super::clone_form::CloneForm;
use super::new_branch_form::NewBranchForm;
use super::repo_picker::{RepoEntry, RepoPicker};
use crate::shared::branch_name::sanitize_branch_name;
use crate::workspace::{Popover, Workspace};

impl Workspace {
    pub fn repo_picker(&mut self, cx: &mut Context<Self>) -> RepoPicker {
        let items = self.filtered_repos(cx);
        self.popover_selected = self.popover_selected.min(items.len().saturating_sub(1));
        let entries = items
            .into_iter()
            .filter_map(|ix| {
                let path = self.settings.repositories.get(ix)?;
                Some(RepoEntry {
                    ix,
                    name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    parent: path.parent().map(|p| p.display().to_string()).unwrap_or_default(),
                    current: self.settings.selected == Some(ix),
                })
            })
            .collect();
        RepoPicker::new(entries, self.popover_selected, self.filter.clone(), self.popover_scroll.clone(), cx.entity())
    }

    /// `None` when no repository is open.
    pub fn branch_picker(&mut self, cx: &mut Context<Self>) -> Option<BranchPicker> {
        let repo = self.repo.clone()?;
        let items = self.filtered_branches(cx);
        self.popover_selected = self.popover_selected.min(items.len().saturating_sub(1));
        Some(BranchPicker::new(
            items,
            self.popover_selected,
            repo,
            self.filter.clone(),
            self.popover_scroll.clone(),
            cx.entity(),
        ))
    }

    pub fn new_branch_form(&self, cx: &mut Context<Self>) -> NewBranchForm {
        NewBranchForm::new(
            sanitize_branch_name(self.name_input.read(cx).as_str()),
            self.repo.as_ref().map(|r| r.read(cx).branch_name()).unwrap_or_default(),
            self.name_input.clone(),
            cx.entity(),
        )
    }

    pub fn clone_form(&self, cx: &mut Context<Self>) -> CloneForm {
        CloneForm::new(
            self.url_input.read(cx).as_str().trim().is_empty(),
            self.url_input.clone(),
            cx.entity(),
        )
    }

    /// Move the picker cursor by `delta` rows and keep it visible.
    pub(super) fn step_picker(&mut self, delta: isize, cx: &mut Context<Self>) {
        self.popover_selected = (self.popover_selected as isize + delta).max(0) as usize;
        self.popover_scroll.scroll_to_item(self.popover_selected, gpui::ScrollStrategy::Nearest);
        cx.notify();
    }

    /// Enter in the repository picker: open the highlighted repository.
    pub(super) fn enter_repo_picker(&mut self, cx: &mut Context<Self>) {
        if let Some(&ix) = self.filtered_repos(cx).get(self.popover_selected) {
            self.open_repository(ix, cx)
        }
    }

    /// Enter in the branch picker: check out the highlighted branch.
    pub(super) fn enter_branch_picker(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else { return };
        let Some(&ix) = self.filtered_branches(cx).get(self.popover_selected) else { return };
        let b = repo.read(cx).branches[ix].clone();
        if !b.is_head {
            repo.update(cx, |r, cx| r.checkout(b, cx));
        }
        self.close_popover(w, cx);
    }

    /// Click on a branch row.
    pub(super) fn pick_branch(&mut self, b: gd_git::Branch, w: &mut Window, cx: &mut Context<Self>) {
        if let (false, Some(repo)) = (b.is_head, &self.repo) {
            repo.update(cx, |r, cx| r.checkout(b, cx));
        }
        self.close_popover(w, cx);
    }

    /// "New branch" footer button: the filter text prefills the name.
    pub(super) fn open_new_branch_from_filter(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let prefill = self.filter.read(cx).as_str().to_string();
        self.toggle_popover(Popover::NewBranch, w, cx);
        self.name_input.update(cx, |s, cx| s.emplace(&prefill, cx));
    }

    pub(super) fn submit_new_branch(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let name = sanitize_branch_name(self.name_input.read(cx).as_str());
        if name.is_empty() {
            return;
        }
        if let Some(repo) = &self.repo {
            repo.update(cx, |r, cx| r.create_branch(name, cx));
        }
        self.close_popover(w, cx);
    }

    pub(super) fn submit_clone(&mut self, cx: &mut Context<Self>) {
        let url = self.url_input.read(cx).as_str().to_string();
        self.clone_repository(url, cx)
    }
}
