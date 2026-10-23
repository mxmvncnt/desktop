//! Which pane a diff is shown in.

use gpui::Entity;

use crate::repo::RepoModel;

/// Which pane a diff is shown in: decides line selection and expansion.
pub enum DiffPane {
    /// Working-tree diff: lines can be selected and hunks expanded.
    Changes(Entity<RepoModel>),
    /// Commit diff: hunks can be expanded.
    History(Entity<RepoModel>),
    /// Stash diff: view only.
    ReadOnly,
}
