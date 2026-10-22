//! Stash state: the list/selection model (`RepoModel::stashes`) and the
//! Workspace-side dialog state ([`StashUi`]).

use std::sync::Arc;

use gd_git::{Branch, Diff, FileChange, Stash};
use gpui::{App, AppContext, Entity, Task, UniformListScrollHandle};
use gpui_elements::editable_text::{EditableTextState, StringStorage};

use crate::diff_highlight::{DiffHighlight, HighlightCache};

#[derive(Default)]
pub struct StashState {
    /// `git stash list`, newest first.
    pub list: Vec<Stash>,
    /// Show stashes made on other branches too (default: this branch only).
    pub all_branches: bool,
    /// Panel collapsed (kept for the session, per repository).
    pub collapsed: bool,
    /// The main pane shows the selected stash instead of the working diff.
    pub viewing: bool,
    /// Sha of the selected stash (indices shift, shas don't).
    pub selected: Option<String>,
    pub files: Vec<FileChange>,
    pub selected_file: Option<usize>,
    pub diff: Option<Arc<Diff>>,
    pub hl: Option<Arc<DiffHighlight>>,
    pub files_scroll: UniformListScrollHandle,
    pub list_scroll: UniformListScrollHandle,
    pub(super) gen_: u64,
    pub(super) task: Option<Task<()>>,
    pub(super) hl_task: Option<Task<()>>,
    pub(super) cache: HighlightCache,
}

impl StashState {
    /// Indices into `list` visible under the current filter.
    pub fn visible(&self, current_branch: Option<&str>) -> Vec<usize> {
        self.list
            .iter()
            .enumerate()
            .filter(|(_, s)| self.all_branches || (s.branch.is_some() && s.branch.as_deref() == current_branch))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn selected_stash(&self) -> Option<&Stash> {
        let sha = self.selected.as_ref()?;
        self.list.iter().find(|s| &s.sha == sha)
    }
}

#[derive(Clone)]
pub enum StashDialog {
    /// New stash. `selected_only` stashes just the checked files.
    Create { selected_only: bool },
    Rename(Stash),
    Drop(Stash),
    /// Checkout with uncommitted changes: `stash` = leave them behind.
    Switch { branch: Branch, stash: bool },
}

/// The one Workspace field for stash UI. It stays a plain struct (not an
/// `Entity`) because Esc/Enter routing in workspace.rs and menus.rs consults
/// `dialog` directly.
pub struct StashUi {
    pub dialog: Option<StashDialog>,
    pub input: Entity<EditableTextState>,
}

impl StashUi {
    pub fn new(cx: &mut App) -> Self {
        Self {
            dialog: None,
            input: cx.new(|cx| EditableTextState::new(StringStorage::default(), cx)),
        }
    }
}
