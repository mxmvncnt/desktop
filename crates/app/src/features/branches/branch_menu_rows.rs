//! Rows of the branch right-click menu.

use gd_git::{Branch, BranchKind};
use gd_ui::Icon;

use super::state::BranchAction as A;
use crate::shared::menu_row::{MenuRow, RowItem};

pub type BranchRow = MenuRow<A>;

/// `paused`: a merge-like operation is in progress (no new merge / rebase).
pub fn branch_menu_rows(b: &Branch, paused: bool) -> Vec<BranchRow> {
    let remote = b.kind == BranchKind::Remote;
    let no_merge = b.is_head || paused;
    vec![
        item(A::Merge, "Merge into current branch…", Icon::Fork, no_merge),
        item(A::Rebase, "Rebase onto this branch…", Icon::Restart, no_merge),
        MenuRow::Separator,
        item(A::CreateFrom, "Create branch from this…", Icon::Plus, false),
        item(A::Rename, "Rename…", Icon::Pen, remote),
        item(A::CopyName, "Copy branch name", Icon::Copy, false),
        MenuRow::Separator,
        // git refuses to delete the checked-out branch.
        MenuRow::Item(
            RowItem::new(A::Delete, if remote { "Delete remote branch…" } else { "Delete…" }, Icon::Trash)
                .danger()
                .disabled(b.is_head),
        ),
    ]
}

fn item(action: A, label: &str, icon: Icon, disabled: bool) -> BranchRow {
    MenuRow::Item(RowItem::new(action, label, icon).disabled(disabled))
}
