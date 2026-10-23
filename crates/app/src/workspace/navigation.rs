//! `impl Workspace`: keyboard selection (↑/↓/Home/End/PageUp/PageDown) in the
//! sidebar and detail lists. The target list is the one last clicked
//! ([`NavList`], set by [`nav_focus`]); diff loads stay off-thread in the
//! `select_*` methods, which drop stale results (latest wins).

use gpui::{Context, Entity, InteractiveElement, ScrollStrategy, Window};

use super::state::Workspace;
use crate::repo::{RepoModel, Tab};

/// Rows moved by PageUp / PageDown.
pub const PAGE: isize = 10;

/// A list that keyboard navigation can drive.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NavList {
    Changes,
    Commits,
    CommitFiles,
    Stashes,
    StashFiles,
    PrFiles,
}

#[derive(Clone, Copy)]
pub enum Move {
    By(isize),
    First,
    Last,
}

/// Make clicks inside `el` the target of arrow-key navigation. Runs in the
/// capture phase so row handlers can't swallow it, and moves focus to the
/// workspace root so a focused text input stops receiving the arrows.
pub fn nav_focus<E: InteractiveElement>(el: E, list: NavList, ws: Entity<Workspace>) -> E {
    el.capture_any_mouse_down(move |_, w, cx| {
        ws.update(cx, |ws, cx| {
            ws.nav_list = Some(list);
            ws.focus(w, cx);
        })
    })
}

impl Workspace {
    pub(super) fn select_move(&mut self, mv: Move, window: &mut Window, cx: &mut Context<Self>) {
        // Another element (a text input) owns the keyboard.
        if window.focused(cx).is_some_and(|h| h != self.focus) {
            return;
        }
        if self.popover.is_some() {
            if let Move::By(d) = mv {
                self.step_popover_selection(d.signum(), cx);
            }
            return;
        }
        if self.overlay_open() {
            return;
        }
        let Some(repo) = self.repo.clone() else { return };
        let list = self.effective_nav_list(repo.read(cx));
        repo.update(cx, |r, cx| match list {
            NavList::Changes => {
                if let Some(ix) = target(r.selected_change, r.changes().len(), mv) {
                    r.select_change(ix, cx);
                    self.changes_scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
                }
            }
            NavList::Commits => {
                if let Some(ix) = target(r.selected_commit, r.commits.len(), mv) {
                    r.select_commit(ix, cx);
                    self.history_scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
                    if ix + 20 >= r.commits.len() {
                        r.load_more_history(cx);
                    }
                }
            }
            NavList::CommitFiles => {
                if let Some(ix) = target(r.selected_commit_file, r.commit_files.len(), mv) {
                    r.select_commit_file(ix, cx);
                    self.commit_files_scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
                }
            }
            NavList::Stashes => {
                let branch = r.status.as_ref().and_then(|s| s.branch.clone());
                let visible = r.stashes.visible(branch.as_deref());
                let cur = r
                    .stashes
                    .selected
                    .as_ref()
                    .filter(|_| r.stashes.viewing)
                    .and_then(|sha| visible.iter().position(|&i| &r.stashes.list[i].sha == sha));
                if let Some(pos) = target(cur, visible.len(), mv) {
                    let sha = r.stashes.list[visible[pos]].sha.clone();
                    r.select_stash(sha, cx);
                    r.stashes.list_scroll.scroll_to_item(pos, ScrollStrategy::Nearest);
                }
            }
            NavList::PrFiles => {
                let n = r.pr.comparison.as_ref().map_or(0, |c| c.files.len());
                if let Some(ix) = target(r.pr.selected_file, n, mv) {
                    r.select_pr_file(ix, cx);
                    r.pr.files_scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
                }
            }
            NavList::StashFiles => {
                if let Some(ix) = target(r.stashes.selected_file, r.stashes.files.len(), mv) {
                    r.select_stash_file(ix, cx);
                    r.stashes.files_scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
                }
            }
        });
    }

    /// The clicked list if it is still on screen, else the tab's main list.
    fn effective_nav_list(&self, r: &RepoModel) -> NavList {
        if r.pr.open {
            return NavList::PrFiles;
        }
        match (r.tab, self.nav_list) {
            (Tab::Changes, Some(NavList::Stashes)) if r.stashes.list.is_empty() => NavList::Changes,
            (Tab::Changes, Some(l @ (NavList::Changes | NavList::Stashes))) => l,
            (Tab::Changes, Some(NavList::StashFiles)) if r.stashes.viewing => NavList::StashFiles,
            (Tab::History, Some(l @ (NavList::Commits | NavList::CommitFiles))) => l,
            (Tab::Changes, _) => NavList::Changes,
            (Tab::History, _) => NavList::Commits,
        }
    }
}

/// New selection index after `mv`, clamped to the list.
fn target(cur: Option<usize>, len: usize, mv: Move) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let last = len as isize - 1;
    Some(match mv {
        Move::First => 0,
        Move::Last => last as usize,
        // From no selection, a downward move lands on the first row and an upward one on the last.
        Move::By(d) => match cur {
            Some(c) => (c as isize + d).clamp(0, last) as usize,
            None if d < 0 => last as usize,
            None => 0,
        },
    })
}
