//! Context menus and confirm dialogs for the Changes list, built on
//! `gd_ui::{ContextMenu, Dialog}`. Only rendered while open (no idle cost).

use std::path::Path;

use gd_git::{FileChange, FileStatus, escape_ignore};
use gd_ui::{ContextMenu, Dialog, Icon, MenuEntry, MenuItem};
use gpui::{
    ClipboardItem, Context, IntoElement, Pixels, Point, Window,
};

use crate::shared::menu_row::{MenuRow, RowItem};
use crate::workspace::Workspace;

/// What was right-clicked.
#[derive(Clone)]
pub enum MenuTarget {
    /// A row of the changes list (index into `RepoModel::changes`, resolved
    /// to the file at open time).
    File(FileChange),
    /// The "N changed files" header.
    Header,
    /// A row of the stash list.
    Stash(gd_git::Stash),
}

pub struct OpenMenu {
    pub position: Point<Pixels>,
    pub target: MenuTarget,
    /// Index into the row list (keyboard highlight).
    pub selected: Option<usize>,
}

pub enum ConfirmDialog {
    DiscardFile(FileChange),
    DiscardAll(Vec<FileChange>),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuAction {
    Discard,
    DiscardAll,
    Ignore,
    IgnoreExt,
    CopyPath,
    CopyRelPath,
    Reveal,
    StashAll,
    StashSelected,
    StashApply,
    StashPop,
    StashDrop,
    StashRename,
    StashCopy,
}

fn extension(path: &str) -> Option<&str> {
    Path::new(path).extension().and_then(|e| e.to_str())
}

/// Menu layout for a target.
fn rows(target: &MenuTarget) -> Vec<MenuRow> {
    match target {
        MenuTarget::Header => vec![
            MenuRow::item(MenuAction::StashAll, "Stash all changes…", Icon::Archive),
            MenuRow::item(MenuAction::StashSelected, "Stash selected files…", Icon::Archive),
            MenuRow::Separator,
            MenuRow::Item(RowItem::new(MenuAction::DiscardAll, "Discard all changes…", Icon::Trash).danger()),
        ],
        MenuTarget::Stash(_) => crate::features::stashes::stash_menu_rows(),
        MenuTarget::File(change) => file_rows(change),
    }
}

fn file_rows(change: &FileChange) -> Vec<MenuRow> {
    let mut v = vec![
        MenuRow::Item(RowItem::new(MenuAction::Discard, "Discard changes…", Icon::Trash).danger()),
        MenuRow::Separator,
        // Ignoring a tracked file has no effect in git, so only
        // offer it for untracked files.
        MenuRow::Item(
            RowItem::new(MenuAction::Ignore, "Ignore file (add to .gitignore)", Icon::EyeClosed)
                .disabled(change.status != FileStatus::Untracked),
        ),
    ];
    if let Some(ext) = extension(&change.path) {
        v.push(MenuRow::item(MenuAction::IgnoreExt, format!("Ignore all .{ext} files"), Icon::EyeClosed));
    }
    v.push(MenuRow::Separator);
    v.push(MenuRow::item(MenuAction::CopyPath, "Copy file path", Icon::Copy));
    v.push(MenuRow::item(MenuAction::CopyRelPath, "Copy relative file path", Icon::Copy));
    v.push(MenuRow::Separator);
    v.push(MenuRow::item(MenuAction::Reveal, "Reveal in file manager", Icon::Folder));
    v
}

impl Workspace {
    pub fn open_menu(
        &mut self,
        position: Point<Pixels>,
        target: MenuTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.popover = None;
        self.dialog = None;
        self.menu = Some(OpenMenu { position, target, selected: None });
        self.menu_gen += 1;
        // Focus the root so ↑/↓/Enter/Esc reach us.
        self.focus(window, cx);
        cx.notify();
    }

    pub fn close_menu(&mut self, cx: &mut Context<Self>) {
        if self.menu.take().is_some() {
            cx.notify();
        }
    }

    /// Open the menu for list row `ix` (resolved against the live model).
    pub fn open_file_menu(
        &mut self,
        ix: usize,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(change) = self
            .repo
            .as_ref()
            .and_then(|r| r.read(cx).changes().get(ix).cloned())
        else {
            return;
        };
        if change.status == FileStatus::Conflicted {
            return self.open_conflict_menu(change, position, window, cx);
        }
        self.open_menu(position, MenuTarget::File(change), window, cx);
    }

    pub fn open_header_menu(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if self.repo.as_ref().is_some_and(|r| !r.read(cx).changes().is_empty()) {
            self.open_menu(position, MenuTarget::Header, window, cx);
        }
    }

    pub fn menu_step(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.merge_ui.menu.is_some() {
            return self.merge_menu_step(delta, cx);
        }
        let Some(menu) = &mut self.menu else { return };
        let rows = rows(&menu.target);
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

    /// Enter: run the highlighted menu item, or confirm the open dialog.
    pub fn menu_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.stash_ui.dialog.is_some() {
            self.stash_confirm(cx);
            return;
        }
        if self.merge_ui_confirm(window, cx) {
            return;
        }
        if self.dialog.is_some() {
            self.confirm_dialog(cx);
            return;
        }
        let Some(menu) = &self.menu else { return };
        let action = menu
            .selected
            .and_then(|s| rows(&menu.target).into_iter().nth(s))
            .and_then(|r| r.action());
        if let Some(a) = action {
            self.run_menu_action(a, window, cx);
        }
    }

    pub fn run_menu_action(&mut self, action: MenuAction, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.menu.take() else { return };
        cx.notify();
        if self.run_stash_action(action, &menu.target, window, cx) {
            return;
        }
        let Some(repo) = self.repo.clone() else { return };
        let root = repo.read(cx).repo.root().to_path_buf();
        let file = match &menu.target {
            MenuTarget::File(c) => Some(c.clone()),
            MenuTarget::Header | MenuTarget::Stash(_) => None,
        };
        match (action, file) {
            (MenuAction::Discard, Some(c)) => self.dialog = Some(ConfirmDialog::DiscardFile(c)),
            (MenuAction::DiscardAll, _) => {
                let all = repo.read(cx).changes().to_vec();
                if !all.is_empty() {
                    self.dialog = Some(ConfirmDialog::DiscardAll(all));
                }
            }
            (MenuAction::CopyPath, Some(c)) => cx.write_to_clipboard(ClipboardItem::new_string(
                root.join(&c.path).display().to_string(),
            )),
            (MenuAction::CopyRelPath, Some(c)) => {
                cx.write_to_clipboard(ClipboardItem::new_string(c.path))
            }
            (MenuAction::Reveal, Some(c)) => {
                let abs = root.join(&c.path);
                // Deleted files no longer exist: reveal the folder instead.
                let target = if abs.exists() { abs } else { abs.parent().unwrap_or(&root).to_path_buf() };
                cx.reveal_path(&target);
            }
            (MenuAction::Ignore, Some(c)) => {
                repo.update(cx, |r, cx| r.ignore(vec![escape_ignore(&c.path)], format!("Ignored {}", c.path), cx))
            }
            (MenuAction::IgnoreExt, Some(c)) => {
                if let Some(ext) = extension(&c.path) {
                    let pattern = format!("*.{}", escape_ignore(ext).trim_start_matches('/'));
                    repo.update(cx, |r, cx| r.ignore(vec![pattern.clone()], format!("Ignored {pattern}"), cx))
                }
            }
            _ => {}
        }
    }

    pub fn close_dialog(&mut self, cx: &mut Context<Self>) {
        if self.dialog.take().is_some() {
            cx.notify();
        }
    }

    pub fn confirm_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.dialog.take() else { return };
        cx.notify();
        let Some(repo) = self.repo.clone() else { return };
        let changes = match dialog {
            ConfirmDialog::DiscardFile(c) => vec![c],
            ConfirmDialog::DiscardAll(all) => all,
        };
        repo.update(cx, |r, cx| r.discard(changes, cx));
    }

    pub fn render_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let menu = self.menu.as_ref()?;
        let ws = cx.entity();
        let mut m = ContextMenu::new(("context-menu", self.menu_gen), menu.position)
            .selected(menu.selected)
            .on_dismiss({
                let ws = ws.clone();
                move |_, cx| ws.update(cx, |this, cx| this.close_menu(cx))
            });
        for r in rows(&menu.target) {
            m = m.entries([match r {
                MenuRow::Separator => MenuEntry::Separator,
                MenuRow::Item(r) => {
                    let ws = ws.clone();
                    let action = r.action;
                    let mut item = MenuItem::new(r.label)
                        .icon(r.icon)
                        .disabled(r.disabled)
                        .on_select(move |w, cx| ws.update(cx, |this, cx| this.run_menu_action(action, w, cx)));
                    if r.danger {
                        item = item.danger();
                    }
                    MenuEntry::Item(item)
                }
            }]);
        }
        let _ = window;
        Some(m)
    }

    pub fn render_dialog(&mut self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let dialog = self.dialog.as_ref()?;
        let (title, body, label) = match dialog {
            ConfirmDialog::DiscardFile(c) => (
                "Confirm discard changes".to_string(),
                format!("Are you sure you want to discard all changes to {}?", c.path),
                "Discard changes",
            ),
            ConfirmDialog::DiscardAll(all) => (
                "Confirm discard all changes".to_string(),
                format!(
                    "Are you sure you want to discard all changes to {} file{}?",
                    all.len(),
                    crate::workspace::plural(all.len() as u32)
                ),
                "Discard all changes",
            ),
        };
        let ws = cx.entity();
        let ws2 = ws.clone();
        Some(
            Dialog::new("discard-dialog", title)
                .body(body)
                .detail("This can't be undone. Untracked files are deleted permanently.")
                .confirm_label(label)
                .danger()
                .on_confirm(move |_, cx| ws.update(cx, |this, cx| this.confirm_dialog(cx)))
                .on_cancel(move |_, cx| ws2.update(cx, |this, cx| this.close_dialog(cx))),
        )
    }
}
