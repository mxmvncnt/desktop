//! Rows of the commit right-click menu, mirroring Desktop's layout.

use gd_ui::Icon;

use super::state::{HistoryAction as A, MenuCtx};
use crate::shared::menu_row::{MenuRow, RowItem};

pub type HistoryRow = MenuRow<A>;

pub fn commit_menu_rows(c: &MenuCtx) -> Vec<HistoryRow> {
    if !c.multi.is_empty() {
        return multi_rows(c);
    }
    let mut v = head_rows(c);
    v.extend(rewrite_rows(c));
    v.push(MenuRow::Separator);
    v.extend(branch_tag_rows(c));
    v.extend([MenuRow::Separator, item(A::CherryPick, "Cherry-pick commit…", Icon::Fork, c.blocked), MenuRow::Separator]);
    v.extend(copy_rows(c));
    v
}

fn item(action: A, label: impl Into<String>, icon: Icon, disabled: bool) -> HistoryRow {
    MenuRow::Item(RowItem::new(action, label, icon).disabled(disabled))
}

fn multi_rows(c: &MenuCtx) -> Vec<HistoryRow> {
    let n = c.multi.len();
    vec![
        item(A::CherryPick, format!("Cherry-pick {n} commits…"), Icon::Fork, c.blocked),
        item(A::Squash, format!("Squash {n} commits…"), Icon::List, c.blocked || !c.squashable),
    ]
}

/// Amend / undo: HEAD only. Pushed commits are allowed too, behind a
/// force-push warning (`HistoryDialog::PushedRewrite`).
fn head_rows(c: &MenuCtx) -> Vec<HistoryRow> {
    if !c.is_head {
        return Vec::new();
    }
    vec![
        item(A::Amend, "Amend commit…", Icon::Pen, c.blocked),
        item(A::Undo, "Undo commit…", Icon::Restart, c.blocked),
    ]
}

fn rewrite_rows(c: &MenuCtx) -> Vec<HistoryRow> {
    vec![
        item(A::Reset, "Reset to commit…", Icon::ArrowLeft, c.blocked || c.is_head),
        item(A::Checkout, "Checkout commit", Icon::GitBranch, c.blocked || c.is_head),
        item(A::MoveUp, "Move commit up (newer)", Icon::ChevronUp, c.blocked || !c.can_up),
        item(A::MoveDown, "Move commit down (older)", Icon::ChevronDown, c.blocked || !c.can_down),
        item(A::Revert, "Revert changes in commit", Icon::Restart, c.blocked),
    ]
}

fn branch_tag_rows(c: &MenuCtx) -> Vec<HistoryRow> {
    let mut v = vec![
        item(A::CreateBranch, "Create branch from commit", Icon::GitBranch, c.blocked),
        item(A::CreateTag, "Create tag…", Icon::Tag, c.blocked),
    ];
    if !c.tags.is_empty() {
        v.push(MenuRow::Separator);
        for t in &c.tags {
            let row = RowItem::new(A::DeleteTag(t.clone()), format!("Delete tag {t}"), Icon::Trash).danger();
            v.push(MenuRow::Item(row.disabled(c.blocked)));
        }
    }
    v
}

fn copy_rows(c: &MenuCtx) -> Vec<HistoryRow> {
    let mut v = vec![item(A::CopySha, "Copy SHA", Icon::Copy, false)];
    if !c.tags.is_empty() {
        let label = if c.tags.len() > 1 { "Copy tags" } else { "Copy tag" };
        v.push(item(A::CopyTags, label, Icon::Copy, false));
    }
    v.push(item(A::ViewOnGitHub, "View on GitHub", Icon::Globe, c.unpushed || c.web_url.is_none()));
    v
}
