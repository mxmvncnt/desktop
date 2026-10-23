//! What each context-menu action does (stash actions are in features/stashes).

use std::path::Path;

use gd_git::{FileChange, escape_ignore};
use gpui::{ClipboardItem, Context, Entity, Window};

use super::menu_rows::extension;
use super::state::{ConfirmDialog, MenuAction, MenuTarget};
use crate::repo::RepoModel;
use crate::workspace::Workspace;

impl Workspace {
    pub fn run_menu_action(&mut self, action: MenuAction, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.menu.take() else { return };
        cx.notify();
        if self.run_stash_action(action, &menu.target, window, cx) {
            return;
        }
        let Some(repo) = self.repo.clone() else { return };
        match (action, menu.target) {
            (MenuAction::DiscardAll, _) => {
                let all = repo.read(cx).changes().to_vec();
                if !all.is_empty() {
                    self.dialog = Some(ConfirmDialog::DiscardAll(all));
                }
            }
            (action, MenuTarget::File(change)) => self.run_file_action(action, change, &repo, cx),
            _ => {}
        }
    }

    fn run_file_action(&mut self, action: MenuAction, c: FileChange, repo: &Entity<RepoModel>, cx: &mut Context<Self>) {
        let root = repo.read(cx).repo.root().to_path_buf();
        match action {
            MenuAction::Discard => self.dialog = Some(ConfirmDialog::DiscardFile(c)),
            MenuAction::CopyPath => {
                cx.write_to_clipboard(ClipboardItem::new_string(root.join(&c.path).display().to_string()))
            }
            MenuAction::CopyRelPath => cx.write_to_clipboard(ClipboardItem::new_string(c.path)),
            MenuAction::Reveal => reveal(&root, &c.path, cx),
            MenuAction::Ignore => {
                repo.update(cx, |r, cx| r.ignore(vec![escape_ignore(&c.path)], format!("Ignored {}", c.path), cx))
            }
            MenuAction::IgnoreExt => ignore_extension(&c.path, repo, cx),
            _ => {}
        }
    }
}

fn reveal(root: &Path, path: &str, cx: &mut gpui::App) {
    let abs = root.join(path);
    // Deleted files no longer exist: reveal the folder instead.
    let target = if abs.exists() { abs } else { abs.parent().unwrap_or(root).to_path_buf() };
    cx.reveal_path(&target);
}

fn ignore_extension(path: &str, repo: &Entity<RepoModel>, cx: &mut gpui::App) {
    if let Some(ext) = extension(path) {
        let pattern = format!("*.{}", escape_ignore(ext).trim_start_matches('/'));
        repo.update(cx, |r, cx| r.ignore(vec![pattern.clone()], format!("Ignored {pattern}"), cx))
    }
}
