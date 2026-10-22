//! Merge feature: merge / squash / rebase dialog, in-progress banner,
//! conflicted-file hint and menu. Git work is in `gd-git/src/merge.rs` and
//! `progress.rs`; see docs/ARCHITECTURE.md.

mod conflict_handlers;
mod conflict_hint;
mod conflict_menu;
mod conflict_menu_rows;
mod handlers;
mod in_progress_banner;
mod merge_candidate_list;
mod merge_dialog;
mod merge_preview;
mod ops;
mod state;

pub use conflict_hint::ConflictHint;
pub use conflict_menu::ConflictMenuView;
pub use in_progress_banner::InProgressBanner;
pub use merge_dialog::MergeDialogView;
pub(crate) use ops::message_event;
pub use state::{MergeMode, MergeState, MergeUi};
