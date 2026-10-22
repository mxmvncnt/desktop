//! Toolbar popovers: repository list, branch list, new branch, clone.
//! Filter input on top, virtualized list, footer actions. Up/down + Enter work.
//! Popover state (`popover`, `popover_selected`, `filter`, ...) stays on
//! `Workspace`; `handlers.rs` builds the components from it.

mod branch_picker;
mod clone_form;
mod filter;
mod handlers;
mod new_branch_form;
mod picker_shell;
mod repo_picker;
