//! Workspace-side handlers of the merge dialog: open, select, step, confirm
//! and close. The preview is computed on the background executor.

use gd_git::{Branch, BranchKind, MergePreview};
use gpui::{AppContext, Context, Focusable, ScrollStrategy, Window};

use super::state::{MergeDialog, MergeMode};
use crate::workspace::Workspace;

impl Workspace {
    /// A merge-like operation is paused (merge is not allowed then).
    pub(crate) fn paused(&self, cx: &gpui::App) -> bool {
        self.repo.as_ref().is_some_and(|r| r.read(cx).merge.in_progress.is_some())
    }

    /// Branches that can be merged: everything but the current branch and
    /// `origin/HEAD`; remote branches with a local twin are hidden.
    pub(super) fn merge_candidates(&self, cx: &gpui::App) -> Vec<Branch> {
        let Some(repo) = &self.repo else { return Vec::new() };
        let f = self.merge_ui.filter.read(cx).as_str().trim().to_lowercase();
        let r = repo.read(cx);
        let local: std::collections::HashSet<&str> =
            r.branches.iter().filter(|b| b.kind == BranchKind::Local).map(|b| b.name.as_str()).collect();
        r.branches
            .iter()
            .filter(|b| !b.is_head)
            .filter(|b| f.is_empty() || b.name.to_lowercase().contains(&f))
            .filter(|b| {
                b.kind == BranchKind::Local || !b.name.split_once('/').is_some_and(|(_, n)| local.contains(n))
            })
            .cloned()
            .collect()
    }

    pub fn open_merge_dialog(
        &mut self,
        mode: MergeMode,
        preselect: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.repo.is_none() || self.paused(cx) {
            return;
        }
        self.popover = None;
        self.menu = None;
        self.dialog = None;
        self.merge_ui.menu = None;
        self.close_branch_overlays();
        self.merge_ui.filter.update(cx, |s, cx| s.emplace("", cx));
        self.merge_ui.summary_for = None;
        self.merge_ui.dialog = Some(MergeDialog::new(mode));
        let ix = preselect
            .and_then(|name| self.merge_candidates(cx).iter().position(|b| b.name == name))
            .unwrap_or(0);
        self.merge_select(ix, cx);
        self.merge_ui.scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
        window.focus(&self.merge_ui.filter.focus_handle(cx), cx);
        cx.notify();
    }

    /// Highlight candidate `ix` and (re)compute its preview in the background.
    pub fn merge_select(&mut self, ix: usize, cx: &mut Context<Self>) {
        let cands = self.merge_candidates(cx);
        let ix = ix.min(cands.len().saturating_sub(1));
        let branch = cands.get(ix).map(|b| b.name.clone());
        self.reset_squash_summary(branch.as_deref(), cx);
        let Some(d) = &mut self.merge_ui.dialog else { return };
        d.ix = ix;
        d.revision += 1;
        d.preview = None;
        d.loading = branch.is_some();
        let generation = d.revision;
        self.merge_ui.preview_task = None;
        cx.notify();
        let (Some(branch), Some(repo)) = (branch, self.repo.clone()) else { return };
        let git = repo.read(cx).repo.clone();
        self.merge_ui.preview_task = Some(cx.spawn(async move |this, cx| {
            let res = cx.background_spawn(async move { git.merge_preview(&branch) }).await;
            this.update(cx, |this, cx| this.preview_loaded(generation, res, cx)).ok();
        }));
    }

    /// Prefill the squash summary when the highlighted branch changes.
    fn reset_squash_summary(&mut self, branch: Option<&str>, cx: &mut Context<Self>) {
        if self.merge_ui.summary_for.as_deref() == branch {
            return;
        }
        self.merge_ui.summary_for = branch.map(str::to_string);
        let text = match (branch, &self.repo) {
            (Some(b), Some(r)) => gd_git::default_squash_summary(b, &r.read(cx).branch_name()),
            _ => String::new(),
        };
        self.merge_ui.summary.update(cx, |s, cx| s.emplace(&text, cx));
    }

    fn preview_loaded(&mut self, generation: u64, res: gd_git::Result<MergePreview>, cx: &mut Context<Self>) {
        let Some(d) = &mut self.merge_ui.dialog else { return };
        if d.revision != generation {
            return;
        }
        d.loading = false;
        match res {
            Ok(p) => d.preview = Some(p),
            Err(e) => {
                if let Some(r) = &self.repo {
                    r.update(cx, |r, cx| {
                        r.error = Some(e.to_string());
                        cx.notify()
                    });
                }
            }
        }
        cx.notify();
    }

    pub(super) fn merge_step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let n = self.merge_candidates(cx).len();
        let Some(d) = &self.merge_ui.dialog else { return };
        if n == 0 {
            return;
        }
        let ix = (d.ix as isize + delta).clamp(0, n as isize - 1) as usize;
        self.merge_ui.scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
        self.merge_select(ix, cx);
    }

    pub(super) fn merge_set_mode(&mut self, mode: MergeMode, cx: &mut Context<Self>) {
        if let Some(d) = &mut self.merge_ui.dialog {
            d.mode = mode;
            cx.notify();
        }
    }

    /// Whether the dialog's primary action can run (branch has commits to bring in).
    pub(super) fn merge_ready(&self) -> bool {
        self.merge_ui.dialog.as_ref().and_then(|d| d.preview.as_ref()).is_some_and(|p| p.ahead > 0)
    }

    pub fn merge_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.merge_ready() {
            return;
        }
        let Some(d) = self.merge_ui.dialog.take() else { return };
        let (Some(p), Some(repo)) = (d.preview, self.repo.clone()) else { return };
        self.focus(window, cx);
        let summary = self.merge_ui.summary.read(cx).as_str().to_string();
        repo.update(cx, |r, cx| r.start_merge(d.mode, p.branch, summary, cx));
        cx.notify();
    }

    /// Close the conflict menu, else the merge dialog. Returns whether one was open.
    pub fn close_merge_ui(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let open = self.merge_ui.dialog.is_some() || self.merge_ui.menu.is_some();
        // Close one layer at a time: menu first.
        if self.merge_ui.menu.take().is_none() {
            self.merge_ui.dialog = None;
            self.merge_ui.preview_task = None;
            self.focus(window, cx);
        }
        if open {
            cx.notify();
        }
        open
    }
}
