//! History tab: commit context menu, its dialogs, multi-select summary and
//! the commit box's "amend" mode. State lives in [`HistoryUi`] (one field on
//! `Workspace`); git work is in `history_ops.rs` / `gd_git::history_ops`.

use std::path::PathBuf;

use gd_git::{BranchKind, Commit, ResetMode};
use gd_ui::{ContextMenu, Dialog, Icon, MenuEntry, MenuItem, Tabs, Theme, icon, pressable};
use gpui::{
    ClipboardItem, Context, Entity, InteractiveElement, IntoElement, ParentElement, Pixels, Point,
    SharedString, Styled, Subscription, Window, div, prelude::*, px, uniform_list,
};
use gpui_elements::editable_text::{EditableTextState, StringStorage, TextChanged, actions::Enter};

use super::pickers::sanitize_branch_name;
use crate::repo::{RepoModel, Tab};
use crate::workspace::{Workspace, plural, text_field};

// ---- state -------------------------------------------------------------

/// Everything the History feature adds to `Workspace`.
pub struct HistoryUi {
    pub menu: Option<HistMenu>,
    pub menu_gen: usize,
    pub dialog: Option<HistDialog>,
    /// Commit box is amending the last commit of this repository.
    pub amend: Option<AmendState>,
    /// Inputs shared by the dialogs (squash message, branch / tag name).
    pub msg_summary: Entity<EditableTextState>,
    pub msg_desc: Entity<EditableTextState>,
    pub name: Entity<EditableTextState>,
    _subs: Vec<Subscription>,
}

pub struct AmendState {
    pub root: PathBuf,
    pub sha: String,
}

impl HistoryUi {
    pub fn new(cx: &mut Context<Workspace>) -> Self {
        let mk = |cx: &mut Context<Workspace>| cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        let (msg_summary, msg_desc, name) = (mk(cx), mk(cx), mk(cx));
        let subs = [&msg_summary, &msg_desc, &name]
            .into_iter()
            .map(|i| cx.subscribe(i, |_: &mut Workspace, _, _: &TextChanged, cx| cx.notify()))
            .collect();
        Self { menu: None, menu_gen: 0, dialog: None, amend: None, msg_summary, msg_desc, name, _subs: subs }
    }

    pub fn is_open(&self) -> bool {
        self.menu.is_some() || self.dialog.is_some()
    }
}

pub struct HistMenu {
    position: Point<Pixels>,
    selected: Option<usize>,
    ctx: MenuCtx,
}

/// Snapshot of everything the menu rows depend on, taken at open time.
pub struct MenuCtx {
    commit: Commit,
    /// Multi-selection, newest first (empty unless 2+ are selected).
    multi: Vec<Commit>,
    is_head: bool,
    unpushed: bool,
    tags: Vec<String>,
    web_url: Option<String>,
    blocked: bool,
    squashable: bool,
    can_up: bool,
    can_down: bool,
}

pub enum HistDialog {
    Reset { commit: Commit, mode: ResetMode },
    Checkout(Commit),
    AmendPushed(Commit),
    CreateBranch(Commit),
    CreateTag(Commit),
    DeleteTag(String),
    CherryPick { commits: Vec<Commit>, branches: Vec<String>, current: Option<String>, selected: usize },
    Squash { commits: Vec<Commit>, any_pushed: bool },
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum HAction {
    Amend,
    Undo,
    Reset,
    Checkout,
    MoveUp,
    MoveDown,
    Revert,
    CreateBranch,
    CreateTag,
    DeleteTag(String),
    CherryPick,
    Squash,
    CopySha,
    CopyTags,
    ViewOnGitHub,
}

struct Row {
    action: HAction,
    label: String,
    icon: Icon,
    danger: bool,
    disabled: bool,
}

fn row(action: HAction, label: impl Into<String>, icon: Icon, disabled: bool) -> Option<Row> {
    Some(Row { action, label: label.into(), icon, danger: false, disabled })
}

/// Menu layout, mirroring Desktop's commit context menu. `None` = separator.
fn rows(c: &MenuCtx) -> Vec<Option<Row>> {
    if !c.multi.is_empty() {
        let n = c.multi.len();
        return vec![
            row(HAction::CherryPick, format!("Cherry-pick {n} commits…"), Icon::Fork, c.blocked),
            row(HAction::Squash, format!("Squash {n} commits…"), Icon::List, c.blocked || !c.squashable),
        ];
    }
    let mut v = Vec::new();
    if c.is_head {
        v.push(row(HAction::Amend, "Amend commit…", Icon::Pen, c.blocked));
        // Undo is only offered for local commits (Desktop); a pushed commit
        // would need a force push.
        v.push(row(HAction::Undo, "Undo commit…", Icon::Restart, c.blocked || !c.unpushed));
    }
    v.push(row(HAction::Reset, "Reset to commit…", Icon::ArrowLeft, c.blocked || c.is_head || !c.unpushed));
    v.push(row(HAction::Checkout, "Checkout commit", Icon::GitBranch, c.blocked || c.is_head));
    v.push(row(HAction::MoveUp, "Move commit up (newer)", Icon::ChevronUp, c.blocked || !c.unpushed || !c.can_up));
    v.push(row(HAction::MoveDown, "Move commit down (older)", Icon::ChevronDown, c.blocked || !c.unpushed || !c.can_down));
    v.push(row(HAction::Revert, "Revert changes in commit", Icon::Restart, c.blocked));
    v.push(None);
    v.push(row(HAction::CreateBranch, "Create branch from commit", Icon::GitBranch, c.blocked));
    v.push(row(HAction::CreateTag, "Create tag…", Icon::Tag, c.blocked));
    if !c.tags.is_empty() {
        v.push(None);
        for t in &c.tags {
            v.push(Some(Row {
                action: HAction::DeleteTag(t.clone()),
                label: format!("Delete tag {t}"),
                icon: Icon::Trash,
                danger: true,
                disabled: c.blocked,
            }));
        }
    }
    v.push(None);
    v.push(row(HAction::CherryPick, "Cherry-pick commit…", Icon::Fork, c.blocked));
    v.push(None);
    v.push(row(HAction::CopySha, "Copy SHA", Icon::Copy, false));
    if !c.tags.is_empty() {
        v.push(row(HAction::CopyTags, if c.tags.len() > 1 { "Copy tags" } else { "Copy tag" }, Icon::Copy, false));
    }
    v.push(row(
        HAction::ViewOnGitHub,
        "View on GitHub",
        Icon::Globe,
        c.unpushed || c.web_url.is_none(),
    ));
    v
}

fn selectable(r: &Option<Row>) -> bool {
    matches!(r, Some(r) if !r.disabled)
}

/// Commits `[..=last]` have at most one parent: the log order is then the
/// first-parent order that squash/reorder work on.
fn linear_through(commits: &[Commit], last: usize) -> bool {
    commits.iter().take(last + 1).all(|c| c.parents.len() <= 1)
}

impl Workspace {
    // ---- menu --------------------------------------------------------------

    pub fn open_commit_menu(
        &mut self,
        ix: usize,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.repo.clone() else { return };
        // Right-clicking outside the multi-selection selects that row alone.
        if !repo.read(cx).commit_selected(ix) {
            repo.update(cx, |r, cx| r.click_commit(ix, false, false, cx));
        }
        let r = repo.read(cx);
        let Some(commit) = r.commits.get(ix).cloned() else { return };
        let multi_ix = r.multi_indices();
        let multi: Vec<Commit> = multi_ix.iter().filter_map(|&i| r.commits.get(i).cloned()).collect();
        let squashable = multi_ix.len() >= 2
            && multi_ix.windows(2).all(|w| w[1] == w[0] + 1)
            && linear_through(&r.commits, *multi_ix.last().unwrap());
        let ctx = MenuCtx {
            tags: r.history_ops.tags.get(&commit.sha).cloned().unwrap_or_default(),
            unpushed: r.is_unpushed(&commit.sha),
            web_url: r.history_ops.web_url.clone(),
            blocked: r.history_blocked(),
            is_head: ix == 0,
            can_up: ix > 0 && linear_through(&r.commits, ix),
            can_down: ix + 1 < r.commits.len() && linear_through(&r.commits, ix + 1),
            squashable,
            multi,
            commit,
        };
        self.popover = None;
        self.menu = None;
        self.dialog = None;
        self.history_ui.dialog = None;
        self.history_ui.menu = Some(HistMenu { position, selected: None, ctx });
        self.history_ui.menu_gen += 1;
        self.focus(window, cx);
        cx.notify();
    }

    pub fn hist_menu_step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(menu) = &mut self.history_ui.menu else { return };
        let rows = rows(&menu.ctx);
        let n = rows.len() as isize;
        let mut ix = menu.selected.map(|s| s as isize).unwrap_or(if delta > 0 { -1 } else { n });
        for _ in 0..n {
            ix = (ix + delta).rem_euclid(n);
            if selectable(&rows[ix as usize]) {
                menu.selected = Some(ix as usize);
                break;
            }
        }
        cx.notify();
    }

    /// Enter: run the highlighted item / confirm the open dialog. `true` if handled.
    pub fn hist_confirm_key(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.history_ui.dialog.is_some() {
            self.hist_confirm(window, cx);
            return true;
        }
        let Some(menu) = &self.history_ui.menu else { return false };
        let action = menu
            .selected
            .and_then(|s| rows(&menu.ctx).into_iter().nth(s))
            .flatten()
            .filter(|r| !r.disabled)
            .map(|r| r.action);
        if let Some(a) = action {
            self.run_hist_action(a, window, cx);
        }
        true
    }

    /// Esc: close the dialog or menu. `true` if something was open.
    pub fn hist_dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let had = self.history_ui.is_open();
        if had {
            self.history_ui.dialog = None;
            self.history_ui.menu = None;
            self.focus(window, cx);
            cx.notify();
        }
        had
    }

    pub fn run_hist_action(&mut self, action: HAction, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.history_ui.menu.take() else { return };
        cx.notify();
        let Some(repo) = self.repo.clone() else { return };
        let ctx = menu.ctx;
        let commit = ctx.commit.clone();
        let sha = commit.sha.clone();
        let dialog_input = |this: &mut Self, w: &mut Window, cx: &mut Context<Self>, which: &Entity<EditableTextState>| {
            w.focus(&which.focus_handle(cx), cx);
        };
        match action {
            HAction::Amend => {
                if ctx.unpushed {
                    self.start_amend(commit, window, cx);
                } else {
                    self.history_ui.dialog = Some(HistDialog::AmendPushed(commit));
                }
            }
            HAction::Undo => repo.update(cx, |r, cx| r.undo_head_commit(cx)),
            HAction::Reset => {
                self.history_ui.dialog = Some(HistDialog::Reset { commit, mode: ResetMode::Mixed })
            }
            HAction::Checkout => self.history_ui.dialog = Some(HistDialog::Checkout(commit)),
            HAction::MoveUp => repo.update(cx, |r, cx| r.move_commit(sha, true, cx)),
            HAction::MoveDown => repo.update(cx, |r, cx| r.move_commit(sha, false, cx)),
            HAction::Revert => repo.update(cx, |r, cx| r.revert_commit(sha, cx)),
            HAction::CreateBranch => {
                self.history_ui.name.update(cx, |s, cx| s.emplace("", cx));
                self.history_ui.dialog = Some(HistDialog::CreateBranch(commit));
                let input = self.history_ui.name.clone();
                dialog_input(self, window, cx, &input);
            }
            HAction::CreateTag => {
                self.history_ui.name.update(cx, |s, cx| s.emplace("", cx));
                self.history_ui.dialog = Some(HistDialog::CreateTag(commit));
                let input = self.history_ui.name.clone();
                dialog_input(self, window, cx, &input);
            }
            HAction::DeleteTag(name) => self.history_ui.dialog = Some(HistDialog::DeleteTag(name)),
            HAction::CherryPick => {
                let mut commits = if ctx.multi.is_empty() { vec![commit] } else { ctx.multi.clone() };
                commits.reverse(); // oldest first
                let r = repo.read(cx);
                let branches: Vec<String> = r
                    .branches
                    .iter()
                    .filter(|b| b.kind == BranchKind::Local)
                    .map(|b| b.name.clone())
                    .collect();
                let current = r.status.as_ref().and_then(|s| s.branch.clone());
                // Default to the first branch that isn't the current one.
                let selected = branches.iter().position(|b| Some(b) != current.as_ref()).unwrap_or(0);
                self.history_ui.dialog = Some(HistDialog::CherryPick { commits, branches, current, selected });
            }
            HAction::Squash => {
                let mut commits = ctx.multi.clone();
                commits.reverse(); // oldest first
                let any_pushed = {
                    let r = repo.read(cx);
                    commits.iter().any(|c| !r.is_unpushed(&c.sha))
                };
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
                self.history_ui.msg_summary.update(cx, |s, cx| s.emplace(&summary, cx));
                self.history_ui.msg_desc.update(cx, |s, cx| s.emplace(&parts.join("\n\n"), cx));
                self.history_ui.dialog = Some(HistDialog::Squash { commits, any_pushed });
                let input = self.history_ui.msg_summary.clone();
                dialog_input(self, window, cx, &input);
            }
            HAction::CopySha => cx.write_to_clipboard(ClipboardItem::new_string(sha)),
            HAction::CopyTags => cx.write_to_clipboard(ClipboardItem::new_string(ctx.tags.join(" "))),
            HAction::ViewOnGitHub => {
                if let Some(url) = ctx.web_url {
                    cx.open_url(&format!("{url}/commit/{sha}"));
                }
            }
        }
        cx.notify();
    }

    pub fn hist_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else { return };
        let name = self.history_ui.name.read(cx).as_str().to_string();
        let name = sanitize_branch_name(&name);
        let message = {
            let s = self.history_ui.msg_summary.read(cx).as_str().trim().to_string();
            let d = self.history_ui.msg_desc.read(cx).as_str().trim().to_string();
            (s, d)
        };
        // Validate before consuming the dialog.
        match &self.history_ui.dialog {
            Some(HistDialog::CreateBranch(_) | HistDialog::CreateTag(_)) if name.is_empty() => return,
            Some(HistDialog::Squash { .. }) if message.0.is_empty() => return,
            None => return,
            _ => {}
        }
        let Some(dialog) = self.history_ui.dialog.take() else { return };
        self.focus(window, cx);
        cx.notify();
        match dialog {
            HistDialog::Reset { commit, mode } => repo.update(cx, |r, cx| r.reset_to_commit(commit.sha, mode, cx)),
            HistDialog::Checkout(c) => repo.update(cx, |r, cx| r.checkout_commit(c.sha, cx)),
            HistDialog::AmendPushed(c) => self.start_amend(c, window, cx),
            HistDialog::CreateBranch(c) => repo.update(cx, |r, cx| r.create_branch_at(name, c.sha, cx)),
            HistDialog::CreateTag(c) => repo.update(cx, |r, cx| r.create_tag(name, c.sha, cx)),
            HistDialog::DeleteTag(t) => repo.update(cx, |r, cx| r.delete_tag(t, cx)),
            HistDialog::CherryPick { commits, branches, selected, .. } => {
                if let Some(target) = branches.get(selected).cloned() {
                    let shas = commits.into_iter().map(|c| c.sha).collect();
                    repo.update(cx, |r, cx| r.cherry_pick(shas, target, cx));
                }
            }
            HistDialog::Squash { commits, .. } => {
                let shas = commits.into_iter().map(|c| c.sha).collect();
                let msg = if message.1.is_empty() { message.0 } else { format!("{}\n\n{}", message.0, message.1) };
                repo.update(cx, |r, cx| r.squash(shas, msg, cx));
            }
        }
    }

    // ---- amend mode ----------------------------------------------------------

    pub fn amending(&self, cx: &gpui::App) -> bool {
        match (&self.history_ui.amend, &self.repo) {
            (Some(a), Some(r)) => a.root == r.read(cx).repo.root(),
            _ => false,
        }
    }

    /// Put the HEAD commit's message in the commit box and switch it to amend mode.
    pub fn start_amend(&mut self, commit: Commit, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else { return };
        let root = repo.read(cx).repo.root().to_path_buf();
        self.history_ui.amend = Some(AmendState { root, sha: commit.sha });
        self.summary.update(cx, |s, cx| s.emplace(&commit.summary, cx));
        self.description.update(cx, |s, cx| s.emplace(&commit.body, cx));
        repo.update(cx, |r, cx| r.set_tab(Tab::Changes, cx));
        window.focus(&self.summary.focus_handle(cx), cx);
        cx.notify();
    }

    pub fn stop_amend(&mut self, cx: &mut Context<Self>) {
        if self.history_ui.amend.take().is_some() {
            self.summary.update(cx, |s, cx| s.emplace("", cx));
            self.description.update(cx, |s, cx| s.emplace("", cx));
            cx.notify();
        }
    }

    /// Commit button while amending. `true` if it handled the commit.
    pub fn try_amend(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.amending(cx) {
            return false;
        }
        let Some(repo) = self.repo.clone() else { return true };
        let summary = self.summary.read(cx).as_str().to_string();
        let description = self.description.read(cx).as_str().to_string();
        if summary.trim().is_empty() || repo.read(cx).history_blocked() {
            return true;
        }
        repo.update(cx, |r, cx| r.amend(summary, description, cx));
        self.history_ui.amend = None;
        self.summary.update(cx, |s, cx| s.emplace("", cx));
        self.description.update(cx, |s, cx| s.emplace("", cx));
        self.focus(window, cx);
        true
    }

    // ---- rendering -----------------------------------------------------------

    pub fn render_hist_menu(&mut self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let menu = self.history_ui.menu.as_ref()?;
        let ws = cx.entity();
        let mut m = ContextMenu::new(("hist-menu", self.history_ui.menu_gen), menu.position)
            .selected(menu.selected)
            .on_dismiss({
                let ws = ws.clone();
                move |w, cx| {
                    ws.update(cx, |this, cx| {
                        this.hist_dismiss(w, cx);
                    })
                }
            });
        for r in rows(&menu.ctx) {
            m = m.entries([match r {
                None => MenuEntry::Separator,
                Some(r) => {
                    let ws = ws.clone();
                    let action = r.action.clone();
                    let mut item = MenuItem::new(r.label)
                        .icon(r.icon)
                        .disabled(r.disabled)
                        .on_select(move |w, cx| {
                            ws.update(cx, |this, cx| this.run_hist_action(action.clone(), w, cx))
                        });
                    if r.danger {
                        item = item.danger();
                    }
                    MenuEntry::Item(item)
                }
            }]);
        }
        Some(m)
    }

    pub fn render_hist_dialog(&mut self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let theme = Theme::of(cx).clone();
        let ws = cx.entity();
        let dialog = self.history_ui.dialog.as_ref()?;
        let name_input = self.history_ui.name.clone();
        let name = sanitize_branch_name(name_input.read(cx).as_str());
        let enter = cx.listener(|this, _: &Enter, w, cx| {
            cx.stop_propagation();
            this.hist_confirm(w, cx)
        });

        let short = |c: &Commit| format!("{} {}", c.short_sha, c.summary);
        let mut confirm_disabled = false;
        let mut danger = false;
        let (title, body, detail, label, content): (String, String, Option<String>, &str, Option<gpui::AnyElement>) =
            match dialog {
                HistDialog::Reset { commit, mode } => {
                    let ws2 = ws.clone();
                    let selected = if *mode == ResetMode::Soft { 0 } else { 1 };
                    let tabs = Tabs::new("reset-mode", selected)
                        .tab("Keep changes staged", None)
                        .tab("Keep changes unstaged", None)
                        .on_select(move |ix, _, cx| {
                            ws2.update(cx, |this, cx| {
                                if let Some(HistDialog::Reset { mode, .. }) = &mut this.history_ui.dialog {
                                    *mode = if ix == 0 { ResetMode::Soft } else { ResetMode::Mixed };
                                }
                                cx.notify();
                            })
                        });
                    (
                        "Reset to commit".into(),
                        format!("Reset the current branch to {}?", short(commit)),
                        Some(match mode {
                            ResetMode::Soft => "Commits after it are removed from the branch. Their changes stay staged, ready to commit again.",
                            ResetMode::Mixed => "Commits after it are removed from the branch. Their changes stay in your working directory, unstaged.",
                        }.to_string()),
                        "Reset",
                        Some(div().pt(px(4.0)).child(tabs).into_any_element()),
                    )
                }
                HistDialog::Checkout(c) => (
                    "Checkout commit".into(),
                    format!("Check out {}?", short(c)),
                    Some("This puts you in a detached HEAD state: commits you make won't belong to any branch until you create one. Your current branch is left untouched.".into()),
                    "Checkout",
                    None,
                ),
                HistDialog::AmendPushed(c) => (
                    "Amend a pushed commit".into(),
                    format!("{} has already been pushed to the remote.", short(c)),
                    Some("Amending rewrites it, so your branch will diverge from the remote and you'll need to force push, which can disrupt anyone who pulled it.".into()),
                    "Amend anyway",
                    None,
                ),
                HistDialog::DeleteTag(t) => {
                    danger = true;
                    (
                        "Delete tag".into(),
                        format!("Delete the tag {t}?"),
                        Some("Only the local tag is deleted. If it was already pushed, it stays on the remote.".into()),
                        "Delete tag",
                        None,
                    )
                }
                HistDialog::CreateBranch(c) => {
                    confirm_disabled = name.is_empty();
                    (
                        "Create branch from commit".into(),
                        format!("Create a branch at {} and switch to it."  , short(c)),
                        None,
                        "Create branch",
                        Some(
                            div()
                                .pt(px(4.0))
                                .capture_action(enter)
                                .child(text_field("hist-name", &name_input, "Branch name", cx))
                                .child(
                                    div().pt(px(6.0)).text_size(px(12.0)).text_color(theme.text_faint).child(
                                        if name.is_empty() { SharedString::from("Choose a name for the new branch") } else { format!("Creates {name}").into() },
                                    ),
                                )
                                .into_any_element(),
                        ),
                    )
                }
                HistDialog::CreateTag(c) => {
                    confirm_disabled = name.is_empty();
                    (
                        "Create tag".into(),
                        format!("Tag {}.", short(c)),
                        None,
                        "Create tag",
                        Some(
                            div()
                                .pt(px(4.0))
                                .capture_action(enter)
                                .child(text_field("hist-name", &name_input, "Tag name", cx))
                                .into_any_element(),
                        ),
                    )
                }
                HistDialog::CherryPick { commits, branches, current, selected } => {
                    let n = commits.len();
                    let rows_n = branches.len();
                    confirm_disabled = rows_n == 0;
                    let list_branches = branches.clone();
                    let current = current.clone();
                    let sel = *selected;
                    let ws2 = ws.clone();
                    let list = uniform_list("cp-branches", rows_n, move |range, _w, cx| {
                        let theme = Theme::of(cx).clone();
                        range
                            .filter_map(|ix| {
                                let b = list_branches.get(ix)?.clone();
                                let ws = ws2.clone();
                                let is_cur = current.as_deref() == Some(b.as_str());
                                Some(
                                    pressable(("cp-branch", ix), sel == ix, cx)
                                        .flex()
                                        .items_center()
                                        .gap(px(8.0))
                                        .h(px(30.0))
                                        .px(px(8.0))
                                        .rounded(px(6.0))
                                        .child(icon(Icon::GitBranch).size(px(14.0)).text_color(theme.text_faint))
                                        .child(div().flex_1().min_w_0().truncate().text_size(px(13.0)).child(b))
                                        .when(is_cur, |d| {
                                            d.child(div().text_size(px(11.5)).text_color(theme.text_faint).child("current"))
                                        })
                                        .when(sel == ix, |d| {
                                            d.child(icon(Icon::Check).size(px(14.0)).text_color(theme.text))
                                        })
                                        .on_click(move |_, _, cx| {
                                            ws.update(cx, |this, cx| {
                                                if let Some(HistDialog::CherryPick { selected, .. }) =
                                                    &mut this.history_ui.dialog
                                                {
                                                    *selected = ix;
                                                }
                                                cx.notify();
                                            })
                                        }),
                                )
                            })
                            .collect()
                    })
                    .h(px(30.0 * rows_n.clamp(1, 6) as f32));
                    (
                        format!("Cherry-pick {n} commit{}", plural(n as u32)),
                        if n == 1 {
                            format!("Copy {} onto which branch?", short(&commits[0]))
                        } else {
                            format!("Copy these {n} commits onto which branch?")
                        },
                        Some("Switches to the chosen branch and applies the commits on top of it. Conflicts leave the cherry-pick in progress.".into()),
                        "Cherry-pick",
                        Some(div().pt(px(4.0)).child(list).into_any_element()),
                    )
                }
                HistDialog::Squash { commits, any_pushed } => {
                    let n = commits.len();
                    confirm_disabled = self.history_ui.msg_summary.read(cx).as_str().trim().is_empty();
                    let desc = gpui_elements::editable_text::text_area("hist-desc")
                        .state(self.history_ui.msg_desc.downgrade())
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
                    (
                        format!("Squash {n} commits"),
                        "Combine them into one commit with this message.".into(),
                        any_pushed.then(|| "Some of these commits were already pushed. Squashing rewrites history, so you'll need to force push.".to_string()),
                        "Squash commits",
                        Some(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(8.0))
                                .pt(px(4.0))
                                .child(
                                    div()
                                        .capture_action(enter)
                                        .child(text_field("hist-summary", &self.history_ui.msg_summary, "Summary (required)", cx)),
                                )
                                .child(desc)
                                .into_any_element(),
                        ),
                    )
                }
            };

        let (ws_c, ws_x) = (ws.clone(), ws);
        let mut d = Dialog::new("hist-dialog", title)
            .body(body)
            .confirm_label(label)
            .confirm_disabled(confirm_disabled)
            .width(480.0)
            .on_confirm(move |w, cx| ws_c.update(cx, |this, cx| this.hist_confirm(w, cx)))
            .on_cancel(move |w, cx| {
                ws_x.update(cx, |this, cx| {
                    this.hist_dismiss(w, cx);
                })
            });
        if let Some(detail) = detail {
            d = d.detail(detail);
        }
        if danger {
            d = d.danger();
        }
        if let Some(c) = content {
            d = d.child(c);
        }
        Some(d)
    }

    /// Main pane while 2+ commits are selected.
    pub fn render_multiselect_main(&mut self, repo: &Entity<RepoModel>, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = Theme::of(cx).clone();
        let r = repo.read(cx);
        let ixs = r.multi_indices();
        let commits: Vec<Commit> = ixs.iter().filter_map(|&i| r.commits.get(i).cloned()).collect();
        let n = commits.len();
        let pushed = commits.iter().filter(|c| !r.is_unpushed(&c.sha)).count();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .gap(px(4.0))
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_size(px(15.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(format!("{n} commits selected")),
                    )
                    .child(div().text_size(px(12.0)).text_color(theme.text_faint).child(
                        "Right-click to squash or cherry-pick. Ctrl/Cmd-click toggles a commit, Shift-click selects a range.",
                    ))
                    .when(pushed > 0, |d| {
                        d.child(div().text_size(px(12.0)).text_color(theme.warning).child(format!(
                            "{pushed} of them {} already on the remote.",
                            if pushed == 1 { "is" } else { "are" }
                        )))
                    }),
            )
            .child(
                uniform_list("multi-commits", n, move |range, _w, cx| {
                    let theme = Theme::of(cx).clone();
                    range
                        .filter_map(|ix| {
                            let c = commits.get(ix)?;
                            Some(
                                div()
                                    .id(("multi-commit", ix))
                                    .flex()
                                    .items_center()
                                    .gap(px(12.0))
                                    .h(px(32.0))
                                    .px(px(16.0))
                                    .text_size(px(13.0))
                                    .child(
                                        div()
                                            .flex_none()
                                            .w(px(64.0))
                                            .font_family(theme.font_mono.clone())
                                            .text_size(px(12.0))
                                            .text_color(theme.text_faint)
                                            .child(c.short_sha.clone()),
                                    )
                                    .child(div().flex_1().min_w_0().truncate().text_color(theme.text_muted).child(c.summary.clone()))
                                    .child(div().flex_none().text_size(px(12.0)).text_color(theme.text_faint).child(c.author_name.clone())),
                            )
                        })
                        .collect()
                })
                .flex_1(),
            )
            .into_any_element()
    }
}
