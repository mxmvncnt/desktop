//! Workspace-side handlers of the conflicted-file menu: open, keyboard
//! navigation, Enter and running an action.

use gpui::{ClipboardItem, Context, Pixels, Point, Window};

use super::conflict_menu_rows::conflict_menu_rows;
use super::state::{ConflictAction, ConflictMenu, Resolve};
use crate::shared::file_url::file_url;
use crate::workspace::Workspace;

impl Workspace {
    pub fn open_conflict_menu(
        &mut self,
        change: gd_git::FileChange,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.menu = None;
        self.close_branch_menu();
        self.merge_ui.menu = Some(ConflictMenu { position, change, selected: None });
        self.merge_ui.menu_gen += 1;
        self.focus(window, cx);
        cx.notify();
    }

    pub fn conflict_menu_step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(menu) = &mut self.merge_ui.menu else { return };
        let rows = conflict_menu_rows();
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

    /// Enter while the conflict menu is open. Returns whether handled.
    pub fn conflict_menu_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(menu) = &self.merge_ui.menu else { return false };
        let action = menu.selected.and_then(|s| conflict_menu_rows().into_iter().nth(s)).and_then(|r| r.action());
        if let Some(a) = action {
            self.run_conflict_action(a, window, cx);
        }
        true
    }

    pub(super) fn run_conflict_action(&mut self, action: ConflictAction, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.merge_ui.menu.take() else { return };
        cx.notify();
        let Some(repo) = self.repo.clone() else { return };
        let root = repo.read(cx).repo.root().to_path_buf();
        let path = menu.change.path;
        let resolve = match action {
            ConflictAction::Ours => Some(Resolve::Ours),
            ConflictAction::Theirs => Some(Resolve::Theirs),
            ConflictAction::Resolved => Some(Resolve::Marked),
            _ => None,
        };
        if let Some(how) = resolve {
            return repo.update(cx, |r, cx| r.resolve_file(path, how, cx));
        }
        match action {
            ConflictAction::OpenEditor => cx.open_url(&file_url(&root.join(&path))),
            ConflictAction::CopyPath => {
                cx.write_to_clipboard(ClipboardItem::new_string(root.join(&path).display().to_string()))
            }
            ConflictAction::Reveal => {
                let abs = root.join(&path);
                cx.reveal_path(&if abs.exists() { abs } else { root });
            }
            ConflictAction::Ours | ConflictAction::Theirs | ConflictAction::Resolved => {}
        }
    }
}
