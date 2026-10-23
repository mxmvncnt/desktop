//! Root view. Layout mirrors GitHub Desktop:
//!
//! ```text
//! ┌ toolbar: [Current repository ▾] [Current branch ▾] [Fetch/Pull/Push] ┐
//! ├ sidebar (300px)           │ main                                    │
//! │  [Changes | History]      │  diff of selected file / commit        │
//! │  file list / commit list  │                                         │
//! │  commit box               │                                         │
//! └───────────────────────────┴─────────────────────────────────────────┘
//! ```
//!
//! `state.rs` has the `Workspace` struct, `render.rs` the root template, and
//! the other files are `impl Workspace` handlers grouped by concern.

mod actions;
mod init;
mod navigation;
mod render;
mod repo_commands;
mod repos;
mod state;

pub use navigation::{NavList, nav_focus};
pub use state::{Popover, Workspace};
