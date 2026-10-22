//! Row layout of the context menu for each target.

use std::path::Path;

use gd_git::{FileChange, FileStatus};
use gd_ui::Icon;

use super::state::{MenuAction, MenuTarget};
use crate::shared::menu_row::{MenuRow, RowItem};

pub fn extension(path: &str) -> Option<&str> {
    Path::new(path).extension().and_then(|e| e.to_str())
}

/// Menu layout for a target.
pub fn menu_rows(target: &MenuTarget) -> Vec<MenuRow> {
    match target {
        MenuTarget::Header => header_rows(),
        MenuTarget::Stash(_) => crate::features::stashes::stash_menu_rows(),
        MenuTarget::File(change) => file_rows(change),
    }
}

fn header_rows() -> Vec<MenuRow> {
    vec![
        MenuRow::item(MenuAction::StashAll, "Stash all changes…", Icon::Archive),
        MenuRow::item(MenuAction::StashSelected, "Stash selected files…", Icon::Archive),
        MenuRow::Separator,
        MenuRow::Item(RowItem::new(MenuAction::DiscardAll, "Discard all changes…", Icon::Trash).danger()),
    ]
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
