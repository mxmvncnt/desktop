//! Context menu of the Changes list (file rows, "N changed files" header,
//! stash rows), the discard confirm dialog, and the Esc/Enter/arrow routing
//! across every open menu and dialog (`key_routing.rs`).
//! Built on `gd_ui::{ContextMenu, Dialog}`; only rendered while open.

mod context_menu_view;
mod discard_dialog;
mod handlers;
mod key_routing;
mod menu_actions;
mod menu_rows;
mod state;

pub use context_menu_view::ContextMenuView;
pub use discard_dialog::DiscardDialogView;
pub use state::{ConfirmDialog, MenuAction, MenuTarget, OpenMenu};
