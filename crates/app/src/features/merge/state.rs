//! Merge state: the model side ([`MergeState`], a field of `RepoModel`) and the
//! Workspace side ([`MergeUi`]: merge dialog, conflicted-file menu, inputs).

use gd_git::{FileChange, InProgress, MergePreview};
use gpui::{AppContext, Context, Entity, Pixels, Point, Subscription, Task, UniformListScrollHandle};
use gpui_elements::editable_text::{EditableTextState, StringStorage, TextChanged};

use crate::workspace::Workspace;

/// Which flavor of "merge into current branch" to run. The discriminant is the
/// index of the dialog tab.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MergeMode {
    Merge,
    Squash,
    Rebase,
}

impl MergeMode {
    pub const ALL: [MergeMode; 3] = [MergeMode::Merge, MergeMode::Squash, MergeMode::Rebase];
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resolve {
    Ours,
    Theirs,
    /// Keep the working-tree contents (after editing by hand).
    Marked,
}

/// State added to `RepoModel` by the merge/conflict feature.
#[derive(Default)]
pub struct MergeState {
    /// Paused operation detected on the last refresh.
    pub in_progress: Option<InProgress>,
    /// A merge/continue/abort is running (disables the banner buttons).
    pub busy: Option<&'static str>,
}

/// The "Merge into current branch" dialog.
pub struct MergeDialog {
    pub mode: MergeMode,
    /// Highlighted row in the filtered candidate list.
    pub ix: usize,
    pub preview: Option<MergePreview>,
    pub loading: bool,
    /// Bumped on each selection; a late preview for an old one is dropped.
    pub(super) revision: u64,
}

impl MergeDialog {
    pub fn new(mode: MergeMode) -> Self {
        Self { mode, ix: 0, preview: None, loading: false, revision: 0 }
    }
}

/// Right-click menu of a conflicted file row.
pub struct ConflictMenu {
    pub position: Point<Pixels>,
    pub change: FileChange,
    pub selected: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConflictAction {
    Ours,
    Theirs,
    Resolved,
    OpenEditor,
    CopyPath,
    Reveal,
}

/// Everything the merge feature adds to `Workspace`. A plain struct (not an
/// `Entity`): Esc/Enter/arrow routing in workspace.rs/menus.rs reads `menu`/`dialog`.
pub struct MergeUi {
    pub dialog: Option<MergeDialog>,
    pub menu: Option<ConflictMenu>,
    pub(super) menu_gen: usize,
    pub filter: Entity<EditableTextState>,
    pub(super) scroll: UniformListScrollHandle,
    pub(super) preview_task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl MergeUi {
    pub fn new(cx: &mut Context<Workspace>) -> Self {
        let filter = cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        let subs = vec![cx.subscribe(&filter, |this: &mut Workspace, _, _: &TextChanged, cx| {
            if this.merge_ui.dialog.is_some() {
                this.merge_select(0, cx);
            }
        })];
        Self {
            dialog: None,
            menu: None,
            menu_gen: 0,
            filter,
            scroll: UniformListScrollHandle::new(),
            preview_task: None,
            _subs: subs,
        }
    }

    /// Whether the root should carry the "Menu" key context (↑/↓/Enter/Esc
    /// reach the workspace instead of an input).
    pub fn wants_menu_context(&self) -> bool {
        self.menu.is_some()
    }
}
