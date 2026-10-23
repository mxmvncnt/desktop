//! Workspace-side handlers of the commit menu: open, keyboard navigation,
//! Enter and Esc. Components call these through their `Entity<Workspace>` prop.

use gd_git::Commit;
use gpui::{Context, Pixels, Point, Window};

use super::commit_menu_rows::commit_menu_rows;
use super::state::{ClickKind, HistoryDialog, HistoryMenu, MenuCtx};
use crate::repo::RepoModel;
use crate::workspace::Workspace;

impl Workspace {
    pub fn open_commit_menu(&mut self, ix: usize, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else { return };
        // Right-clicking outside the multi-selection selects that row alone.
        if !repo.read(cx).commit_selected(ix) {
            repo.update(cx, |r, cx| r.click_commit(ix, ClickKind::Plain, cx));
        }
        let Some(ctx) = menu_ctx(repo.read(cx), ix) else { return };
        self.popover = None;
        self.menu = None;
        self.dialog = None;
        self.history_ui.dialog = None;
        self.history_ui.menu = Some(HistoryMenu { position, selected: None, ctx });
        self.history_ui.menu_gen += 1;
        self.focus(window, cx);
        cx.notify();
    }

    /// Arrow keys: move the highlight, skipping separators and disabled rows.
    pub fn hist_menu_step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(menu) = &mut self.history_ui.menu else { return };
        let rows = commit_menu_rows(&menu.ctx);
        let n = rows.len() as isize;
        let mut ix = menu.selected.map(|s| s as isize).unwrap_or(if delta > 0 { -1 } else { n });
        for _ in 0..n {
            ix = (ix + delta).rem_euclid(n);
            if rows[ix as usize].is_selectable() {
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
        let action = menu.selected.and_then(|s| commit_menu_rows(&menu.ctx).into_iter().nth(s)).and_then(|r| r.action());
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
}

/// Snapshot of everything the menu rows depend on, for the commit at `ix`.
fn menu_ctx(r: &RepoModel, ix: usize) -> Option<MenuCtx> {
    let commit = r.commits.get(ix).cloned()?;
    let multi_ix = r.multi_indices();
    let multi = multi_ix.iter().filter_map(|&i| r.commits.get(i).cloned()).collect();
    let squashable = multi_ix.len() >= 2
        && multi_ix.windows(2).all(|w| w[1] == w[0] + 1)
        && linear_through(&r.commits, *multi_ix.last().unwrap());
    Some(MenuCtx {
        tags: r.history.tags.get(&commit.sha).cloned().unwrap_or_default(),
        unpushed: r.is_unpushed(&commit.sha),
        web_url: r.history.web_url.clone(),
        blocked: r.history_blocked(),
        is_head: ix == 0,
        can_up: ix > 0 && linear_through(&r.commits, ix),
        can_down: ix + 1 < r.commits.len() && linear_through(&r.commits, ix + 1),
        squashable,
        multi,
        commit,
    })
}

/// Commits `[..=last]` have at most one parent: the log order is then the
/// first-parent order that squash/reorder work on.
fn linear_through(commits: &[Commit], last: usize) -> bool {
    commits.iter().take(last + 1).all(|c| c.parents.len() <= 1)
}

impl Workspace {
    /// Reset dialog: the Keep-staged / Keep-unstaged tabs.
    pub(super) fn set_reset_mode(&mut self, new_mode: gd_git::ResetMode, cx: &mut Context<Self>) {
        if let Some(HistoryDialog::Reset { mode, .. }) = &mut self.history_ui.dialog {
            *mode = new_mode;
        }
        cx.notify();
    }

    /// Cherry-pick dialog: choose the target branch row.
    pub(super) fn set_cherry_pick_branch(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some(HistoryDialog::CherryPick { target, .. }) = &mut self.history_ui.dialog {
            target.selected = ix;
        }
        cx.notify();
    }
}
