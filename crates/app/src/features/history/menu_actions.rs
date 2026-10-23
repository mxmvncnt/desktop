//! Running a commit-menu action: either straight on the model, or by opening
//! the dialog that asks for the missing input / confirmation.

use gd_git::{BranchKind, Commit, ResetMode};
use gpui::{ClipboardItem, Context, Entity, Focusable, Window};
use gpui_elements::editable_text::EditableTextState;

use super::state::{CherryPickTarget, HistoryAction, HistoryDialog, MenuCtx};
use crate::repo::RepoModel;
use crate::workspace::Workspace;

impl Workspace {
    pub fn run_hist_action(&mut self, action: HistoryAction, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.history_ui.menu.take() else { return };
        cx.notify();
        let Some(repo) = self.repo.clone() else { return };
        let ctx = menu.ctx;
        let commit = ctx.commit.clone();
        let sha = commit.sha.clone();
        // History-rewriting actions on a pushed commit: warn first.
        if !ctx.unpushed && action.rewrites_history() {
            self.history_ui.dialog = Some(HistoryDialog::PushedRewrite { action, commit });
            return;
        }
        match action {
            HistoryAction::Amend if ctx.unpushed => self.start_amend(commit, window, cx),
            HistoryAction::Amend => self.history_ui.dialog = Some(HistoryDialog::AmendPushed(commit)),
            HistoryAction::Undo => repo.update(cx, |r, cx| r.undo_head_commit(cx)),
            HistoryAction::Reset => self.history_ui.dialog = Some(reset_dialog(commit)),
            HistoryAction::Checkout => self.history_ui.dialog = Some(HistoryDialog::Checkout(commit)),
            HistoryAction::MoveUp => repo.update(cx, |r, cx| r.move_commit(sha, true, cx)),
            HistoryAction::MoveDown => repo.update(cx, |r, cx| r.move_commit(sha, false, cx)),
            HistoryAction::Revert => repo.update(cx, |r, cx| r.revert_commit(sha, cx)),
            HistoryAction::CreateBranch => self.open_name_dialog(HistoryDialog::CreateBranch(commit), window, cx),
            HistoryAction::CreateTag => self.open_name_dialog(HistoryDialog::CreateTag(commit), window, cx),
            HistoryAction::DeleteTag(name) => self.history_ui.dialog = Some(HistoryDialog::DeleteTag(name)),
            HistoryAction::CherryPick => self.open_cherry_pick(ctx, &repo, cx),
            HistoryAction::Squash => self.open_squash(ctx, &repo, window, cx),
            HistoryAction::CopySha => cx.write_to_clipboard(ClipboardItem::new_string(sha)),
            HistoryAction::CopyTags => cx.write_to_clipboard(ClipboardItem::new_string(ctx.tags.join(" "))),
            HistoryAction::ViewOnGitHub => {
                if let Some(url) = ctx.web_url {
                    cx.open_url(&format!("{url}/commit/{sha}"));
                }
            }
        }
        cx.notify();
    }

    /// Branch / tag name dialogs: start with an empty, focused name input.
    fn open_name_dialog(&mut self, dialog: HistoryDialog, window: &mut Window, cx: &mut Context<Self>) {
        self.history_ui.name.update(cx, |s, cx| s.emplace("", cx));
        self.history_ui.dialog = Some(dialog);
        focus_input(&self.history_ui.name.clone(), window, cx);
    }

    fn open_cherry_pick(&mut self, ctx: MenuCtx, repo: &Entity<RepoModel>, cx: &mut Context<Self>) {
        let mut commits = if ctx.multi.is_empty() { vec![ctx.commit] } else { ctx.multi };
        commits.reverse(); // oldest first
        let r = repo.read(cx);
        let branches: Vec<String> =
            r.branches.iter().filter(|b| b.kind == BranchKind::Local).map(|b| b.name.clone()).collect();
        let current = r.status.as_ref().and_then(|s| s.branch.clone());
        // Default to the first branch that isn't the current one.
        let selected = branches.iter().position(|b| Some(b) != current.as_ref()).unwrap_or(0);
        let target = CherryPickTarget { branches, current, selected };
        self.history_ui.dialog = Some(HistoryDialog::CherryPick { commits, target });
    }

    fn open_squash(&mut self, ctx: MenuCtx, repo: &Entity<RepoModel>, window: &mut Window, cx: &mut Context<Self>) {
        let mut commits = ctx.multi;
        commits.reverse(); // oldest first
        let r = repo.read(cx);
        let any_pushed = commits.iter().any(|c| !r.is_unpushed(&c.sha));
        let (summary, description) = squash_message(&commits);
        self.history_ui.msg_summary.update(cx, |s, cx| s.emplace(&summary, cx));
        self.history_ui.msg_desc.update(cx, |s, cx| s.emplace(&description, cx));
        self.history_ui.dialog = Some(HistoryDialog::Squash { commits, any_pushed });
        focus_input(&self.history_ui.msg_summary.clone(), window, cx);
    }
}

pub(super) fn reset_dialog(commit: Commit) -> HistoryDialog {
    HistoryDialog::Reset { commit, mode: ResetMode::Mixed }
}

fn focus_input(input: &Entity<EditableTextState>, window: &mut Window, cx: &mut Context<Workspace>) {
    window.focus(&input.focus_handle(cx), cx);
}

/// Default squash message for `commits` (oldest first): the oldest summary,
/// and the other commits' summaries and bodies as the description.
fn squash_message(commits: &[Commit]) -> (String, String) {
    let summary = commits.first().map(|c| c.summary.clone()).unwrap_or_default();
    let mut parts = Vec::new();
    for (i, c) in commits.iter().enumerate() {
        let mut p = if i == 0 { String::new() } else { c.summary.clone() };
        if !c.body.is_empty() {
            if !p.is_empty() {
                p.push('\n');
            }
            p.push_str(&c.body);
        }
        if !p.is_empty() {
            parts.push(p);
        }
    }
    (summary, parts.join("\n\n"))
}
