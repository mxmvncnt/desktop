//! History feature: commit list, commit detail, context menu, dialogs and
//! amend mode. Git work is in `gd-git/src/history_ops.rs`; see
//! docs/ARCHITECTURE.md.

mod amend;
mod cherry_pick_branch_row;
mod commit_detail;
mod commit_file_list;
mod commit_header;
mod commit_list;
mod commit_menu;
mod commit_menu_rows;
mod commit_row;
mod confirm;
mod handlers;
mod history_dialog;
mod menu_actions;
mod message_handle;
mod message_resize;
mod multi_select;
mod ops;
mod selection;
mod state;

pub use commit_list::CommitList;
pub use commit_menu::CommitMenuView;
pub use history_dialog::HistoryDialogView;
pub use message_resize::MessageResize;
#[cfg(feature = "snapshot")]
pub use state::{ClickKind, HistoryAction};
pub use state::{HistoryState, HistoryUi};
