//! Rows of the conflicted-file right-click menu.

use gd_ui::Icon;

use super::state::ConflictAction as A;
use crate::shared::menu_row::MenuRow;

pub type ConflictRow = MenuRow<A>;

pub fn conflict_menu_rows() -> Vec<ConflictRow> {
    vec![
        MenuRow::item(A::Ours, "Use ours", Icon::ArrowLeft),
        MenuRow::item(A::Theirs, "Use theirs", Icon::ArrowRight),
        MenuRow::item(A::Resolved, "Mark as resolved", Icon::Check),
        MenuRow::Separator,
        MenuRow::item(A::OpenEditor, "Open in default editor", Icon::Document),
        MenuRow::Separator,
        MenuRow::item(A::CopyPath, "Copy file path", Icon::Copy),
        MenuRow::item(A::Reveal, "Reveal in file manager", Icon::Folder),
    ]
}
