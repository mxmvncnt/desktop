//! Branch operations feature: branch context menu and the rename / delete /
//! create-from dialogs. Git work is in `gd-git/src/merge.rs`; the ops run
//! through the merge feature's `RepoModel::run_op`. See docs/ARCHITECTURE.md.

mod branch_dialog;
mod branch_menu;
mod branch_menu_rows;
mod handlers;
mod ops;
mod state;

pub use branch_dialog::BranchDialogView;
pub use branch_menu::BranchMenuView;
pub use state::BranchUi;
