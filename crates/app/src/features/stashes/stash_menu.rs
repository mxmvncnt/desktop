//! Rows of the stash right-click menu (rendered by `views/menus.rs`).

use gd_ui::Icon;

use crate::shared::menu_row::{MenuRow, RowItem};
use crate::views::menus::MenuAction;

pub fn stash_menu_rows() -> Vec<MenuRow> {
    vec![
        MenuRow::item(MenuAction::StashApply, "Apply (keep stash)", Icon::ArchiveUp),
        MenuRow::item(MenuAction::StashPop, "Pop (apply and remove)", Icon::Restart),
        MenuRow::Separator,
        MenuRow::item(MenuAction::StashRename, "Rename…", Icon::Pen),
        MenuRow::item(MenuAction::StashCopy, "Copy message", Icon::Copy),
        MenuRow::Separator,
        MenuRow::Item(RowItem::new(MenuAction::StashDrop, "Drop…", Icon::Trash).danger()),
    ]
}
