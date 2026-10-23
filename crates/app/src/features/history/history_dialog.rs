//! The history dialogs (reset, checkout, pushed-history warnings, branch/tag
//! name, cherry-pick target, squash message).

use gd_git::{Commit, ResetMode};
use gd_ui::{Dialog, Tabs, Theme};
use gpui::{App, Context, Entity, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px, uniform_list};
use gpui_elements::editable_text::{EditableTextState, actions::Enter};

use super::cherry_pick_branch_row::CherryPickBranchRow;
use super::state::{CherryPickTarget, HistoryAction, HistoryDialog};
use crate::shared::branch_name::sanitize_branch_name;
use crate::shared::plural::plural;
use crate::shared::text_field::text_field;
use crate::workspace::Workspace;

const BRANCH_ROW_HEIGHT: f32 = 30.0;
const BRANCH_ROWS_VISIBLE: usize = 6;

#[derive(IntoElement)]
pub struct HistoryDialogView {
    dialog: HistoryDialog,
    ws: Entity<Workspace>,
    name: Entity<EditableTextState>,
    msg_summary: Entity<EditableTextState>,
    msg_desc: Entity<EditableTextState>,
}

impl HistoryDialogView {
    /// `None` when no history dialog is open.
    pub fn open(ws: &Workspace, cx: &mut Context<Workspace>) -> Option<Self> {
        let ui = &ws.history_ui;
        Some(Self {
            dialog: ui.dialog.clone()?,
            ws: cx.entity(),
            name: ui.name.clone(),
            msg_summary: ui.msg_summary.clone(),
            msg_desc: ui.msg_desc.clone(),
        })
    }
}

impl RenderOnce for HistoryDialogView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let dialog = match &self.dialog {
            HistoryDialog::Reset { commit, mode } => self.reset(commit, *mode),
            HistoryDialog::Checkout(c) => checkout(c),
            HistoryDialog::PushedRewrite { action, commit } => pushed_rewrite(action, commit),
            HistoryDialog::AmendPushed(c) => amend_pushed(c),
            HistoryDialog::DeleteTag(t) => delete_tag(t),
            HistoryDialog::CreateBranch(c) => self.create_branch(c, cx),
            HistoryDialog::CreateTag(c) => self.create_tag(c, cx),
            HistoryDialog::CherryPick { commits, target } => self.cherry_pick(commits, target),
            HistoryDialog::Squash { commits, any_pushed } => self.squash(commits.len(), *any_pushed, cx),
        };
        let (ws, ws2) = (self.ws.clone(), self.ws);
        dialog
            .width(480.0)
            .on_confirm(move |w, cx| ws.update(cx, |this, cx| this.hist_confirm(w, cx)))
            .on_cancel(move |w, cx| {
                ws2.update(cx, |this, cx| {
                    this.hist_dismiss(w, cx);
                })
            })
    }
}

impl HistoryDialogView {
    fn reset(&self, commit: &Commit, mode: ResetMode) -> Dialog {
        let ws = self.ws.clone();
        let tabs = Tabs::new("reset-mode", if mode == ResetMode::Soft { 0 } else { 1 })
            .tab("Keep changes staged", None)
            .tab("Keep changes unstaged", None)
            .on_select(move |ix, _, cx| {
                let mode = if ix == 0 { ResetMode::Soft } else { ResetMode::Mixed };
                ws.update(cx, |this, cx| this.set_reset_mode(mode, cx))
            });
        let detail = match mode {
            ResetMode::Soft => "Commits after it are removed from the branch. Their changes stay staged, ready to commit again.",
            ResetMode::Mixed => "Commits after it are removed from the branch. Their changes stay in your working directory, unstaged.",
        };
        Dialog::new("hist-dialog", "Reset to commit")
            .body(format!("Reset the current branch to {}?", short(commit)))
            .detail(detail)
            .confirm_label("Reset")
            .content(div().pt(px(4.0)).child(tabs))
    }

    fn create_branch(&self, c: &Commit, cx: &App) -> Dialog {
        let name = sanitize_branch_name(self.name.read(cx).as_str());
        let hint: SharedString = if name.is_empty() {
            "Choose a name for the new branch".into()
        } else {
            format!("Creates {name}").into()
        };
        let theme = Theme::of(cx);
        let content = self
            .name_field("Branch name", cx)
            .child(div().pt(px(6.0)).text_size(px(12.0)).text_color(theme.text_faint).child(hint));
        Dialog::new("hist-dialog", "Create branch from commit")
            .body(format!("Create a branch at {} and switch to it.", short(c)))
            .confirm_label("Create branch")
            .confirm_disabled(name.is_empty())
            .content(content)
    }

    fn create_tag(&self, c: &Commit, cx: &App) -> Dialog {
        let name = sanitize_branch_name(self.name.read(cx).as_str());
        Dialog::new("hist-dialog", "Create tag")
            .body(format!("Tag {}.", short(c)))
            .confirm_label("Create tag")
            .confirm_disabled(name.is_empty())
            .content(self.name_field("Tag name", cx))
    }

    fn cherry_pick(&self, commits: &[Commit], target: &CherryPickTarget) -> Dialog {
        let n = commits.len();
        let body = if n == 1 {
            format!("Copy {} onto which branch?", short(&commits[0]))
        } else {
            format!("Copy these {n} commits onto which branch?")
        };
        Dialog::new("hist-dialog", format!("Cherry-pick {n} commit{}", plural(n as u32)))
            .body(body)
            .detail("Switches to the chosen branch and applies the commits on top of it. Conflicts leave the cherry-pick in progress.")
            .confirm_label("Cherry-pick")
            .confirm_disabled(target.branches.is_empty())
            .content(div().pt(px(4.0)).child(self.branch_list(target)))
    }

    fn branch_list(&self, target: &CherryPickTarget) -> impl IntoElement + use<> {
        let n = target.branches.len();
        let (branches, current, sel) = (target.branches.clone(), target.current.clone(), target.selected);
        let ws = self.ws.clone();
        uniform_list("cp-branches", n, move |range, _w, _cx| {
            range
                .filter_map(|ix| {
                    let name = branches.get(ix)?.clone();
                    let is_current = current.as_deref() == Some(name.as_str());
                    let ws = ws.clone();
                    let on_select = move |_: &mut Window, cx: &mut App| ws.update(cx, |this, cx| this.set_cherry_pick_branch(ix, cx));
                    Some(CherryPickBranchRow::new(ix, name, sel == ix, is_current, on_select))
                })
                .collect()
        })
        .h(px(BRANCH_ROW_HEIGHT * n.clamp(1, BRANCH_ROWS_VISIBLE) as f32))
    }

    fn squash(&self, n: usize, any_pushed: bool, cx: &App) -> Dialog {
        let theme = Theme::of(cx).clone();
        let desc = gpui_elements::editable_text::text_area("hist-desc")
            .state(self.msg_desc.downgrade())
            .placeholder("Description")
            .placeholder_color(theme.text_faint)
            .selection_color(theme.selection)
            .caret_color(theme.caret)
            .caret_blink_interval_500ms()
            .w_full()
            .h(px(120.0))
            .px(px(10.0))
            .py(px(8.0))
            .rounded(px(gd_ui::theme::CONTROL_RADIUS + 1.0))
            .bg(theme.input_bg)
            .border_1()
            .border_color(theme.border)
            .text_size(px(13.0))
            .text_color(theme.text)
            .overflow_y_scroll();
        let summary = div()
            .capture_action(self.enter_confirms())
            .child(text_field("hist-summary", &self.msg_summary, "Summary (required)", cx));
        let content = div().flex().flex_col().gap(px(8.0)).pt(px(4.0)).child(summary).child(desc);
        let mut d = Dialog::new("hist-dialog", format!("Squash {n} commits"))
            .body("Combine them into one commit with this message.")
            .confirm_label("Squash commits")
            .confirm_disabled(self.msg_summary.read(cx).as_str().trim().is_empty())
            .content(content);
        if any_pushed {
            d = d.detail("Some of these commits were already pushed. Squashing rewrites history, so you'll need to force push.");
        }
        d
    }

    /// Single-line name input; Enter confirms the dialog.
    fn name_field(&self, placeholder: &str, cx: &App) -> gpui::Div {
        div().pt(px(4.0)).capture_action(self.enter_confirms()).child(text_field("hist-name", &self.name, placeholder, cx))
    }

    fn enter_confirms(&self) -> impl Fn(&Enter, &mut Window, &mut App) + 'static {
        let ws = self.ws.clone();
        move |_: &Enter, w, cx| {
            cx.stop_propagation();
            ws.update(cx, |this, cx| this.hist_confirm(w, cx))
        }
    }
}

fn short(c: &Commit) -> String {
    format!("{} {}", c.short_sha, c.summary)
}

fn checkout(c: &Commit) -> Dialog {
    Dialog::new("hist-dialog", "Checkout commit")
        .body(format!("Check out {}?", short(c)))
        .detail("This puts you in a detached HEAD state: commits you make won't belong to any branch until you create one. Your current branch is left untouched.")
        .confirm_label("Checkout")
}

fn pushed_rewrite(action: &HistoryAction, commit: &Commit) -> Dialog {
    let label = match action {
        HistoryAction::Undo => "Undo anyway",
        HistoryAction::Reset => "Continue",
        _ => "Move anyway",
    };
    Dialog::new("hist-dialog", "Rewrite pushed history?")
        .body(format!("{} has already been pushed to the remote.", short(commit)))
        .detail("This rewrites your branch's history, so it will diverge from the remote and you'll need to force push, which can disrupt anyone who already pulled it.")
        .confirm_label(label)
}

fn amend_pushed(c: &Commit) -> Dialog {
    Dialog::new("hist-dialog", "Amend a pushed commit")
        .body(format!("{} has already been pushed to the remote.", short(c)))
        .detail("Amending rewrites it, so your branch will diverge from the remote and you'll need to force push, which can disrupt anyone who pulled it.")
        .confirm_label("Amend anyway")
}

fn delete_tag(t: &str) -> Dialog {
    Dialog::new("hist-dialog", "Delete tag")
        .body(format!("Delete the tag {t}?"))
        .detail("Only the local tag is deleted. If it was already pushed, it stays on the remote.")
        .confirm_label("Delete tag")
        .danger()
}
