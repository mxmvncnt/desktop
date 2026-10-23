//! Changes tab: sidebar (tabs, file list, commit box) and the main pane
//! (selected file's diff, or "No local changes" suggestions).
//! RepoModel ops are in `ops.rs`; the diff itself is `features/diff`.

mod amend_notice;
mod change_row;
mod changes_header;
mod changes_list;
mod changes_pane;
mod changes_panel;
mod commit_box;
mod file_header;
mod no_changes;
mod ops;
mod sidebar;
mod resize;
mod sidebar_handle;
mod sidebar_tabs;
mod state;
mod undo_strip;

pub use sidebar::Sidebar;
pub use sidebar_handle::SidebarResize;
