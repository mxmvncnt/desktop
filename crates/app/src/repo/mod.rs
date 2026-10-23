//! `RepoModel`: all state for the currently-open repository, plus the async
//! plumbing that runs git on the background executor.
//!
//! Rules (see docs/ARCHITECTURE.md §Threading):
//! - Never call `gd_git` on the UI thread. Use [`RepoModel::run`] /
//!   `cx.background_spawn` and apply results in `this.update(..)`.
//! - Stale results are dropped: each load bumps a generation counter and the
//!   completion handler ignores results from an older generation (the user
//!   may have clicked another file while a big diff was loading).
//!
//! Files: `state.rs` (struct + events), `refresh.rs` (coalesced refresh,
//! watcher), `background_fetch.rs`, `change_diff.rs` / `commit_diff.rs`
//! (diff loading with generation counters), `diff_highlight.rs`, `expand.rs`,
//! `history_log.rs` (paged log), `run.rs` (generic write-op helper).
//! Feature-specific ops live in `features/<x>/ops.rs`.

mod background_fetch;
mod change_diff;
mod commit_diff;
mod diff_highlight;
mod expand;
mod history_log;
mod refresh;
mod run;
mod state;

pub use state::{LastCommit, RemoteOp, RepoEvent, RepoModel, Tab};
