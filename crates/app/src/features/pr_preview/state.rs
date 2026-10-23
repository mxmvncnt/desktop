//! Pull-request preview state (`RepoModel::pr`).

use std::sync::Arc;

use gd_git::{Comparison, Diff};
use gpui::{Task, UniformListScrollHandle};

use crate::features::diff::{DiffHighlight, HighlightCache};

#[derive(Default)]
pub struct PrPreviewState {
    /// The main pane shows the preview instead of the tab content.
    pub open: bool,
    /// Chosen base (short branch name); `None` = the repository's default.
    pub base: Option<String>,
    /// The base dropdown is expanded.
    pub base_menu: bool,
    /// Base the loaded `comparison` was made against.
    pub loaded_base: Option<String>,
    pub loading: bool,
    pub comparison: Option<Comparison>,
    pub error: Option<String>,
    /// Fetch URL of the remote the PR would be opened on.
    pub remote_url: Option<String>,
    pub selected_file: Option<usize>,
    pub diff: Option<Arc<Diff>>,
    pub hl: Option<Arc<DiffHighlight>>,
    pub files_scroll: UniformListScrollHandle,
    pub base_scroll: UniformListScrollHandle,
    pub(super) gen_: u64,
    pub(super) task: Option<Task<()>>,
    pub(super) hl_task: Option<Task<()>>,
    pub(super) cache: HighlightCache,
}
