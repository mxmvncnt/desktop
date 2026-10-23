use gpui::{Entity, FocusHandle, Pixels, Point, SharedString, Subscription, Task, UniformListScrollHandle};
use gpui_elements::editable_text::EditableTextState;

use crate::features::accounts::AccountsUi;
use crate::features::branches::BranchUi;
use crate::features::diff::{DiffSelectUi, DiffView};
use crate::features::history::HistoryUi;
use crate::features::menus::{ConfirmDialog, OpenMenu};
use crate::features::merge::MergeUi;
use crate::features::settings::{Settings, SettingsPage};
use crate::features::stashes::StashUi;
use crate::repo::RepoModel;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Popover {
    Repositories,
    Branches,
    NewBranch,
    Clone,
}

pub struct Workspace {
    pub settings: Settings,
    /// Sidebar width changed by a drag and not yet saved.
    pub(crate) sidebar_width_dirty: bool,
    /// Settings modal (features/settings/).
    pub settings_open: bool,
    pub settings_page: SettingsPage,
    /// Git identity + provider accounts (features/accounts).
    pub accounts_ui: AccountsUi,
    pub repo: Option<Entity<RepoModel>>,
    pub popover: Option<Popover>,
    /// Bumped on every popover open so its entrance animation replays.
    pub popover_gen: usize,
    /// Popover the backdrop just dismissed, and where that press landed. The
    /// backdrop doesn't occlude the toolbar, so the same press also clicks
    /// the trigger underneath; this lets the trigger ignore that click
    /// instead of reopening.
    pub(crate) popover_dismissed: Option<(Popover, Point<Pixels>)>,
    pub popover_selected: usize,
    /// New-branch popover: chosen start point (`None` = current branch) and
    /// whether the base list is expanded.
    pub new_branch_base: Option<String>,
    pub new_branch_base_open: bool,
    /// Open right-click menu / confirm dialog (rendered only while `Some`).
    pub menu: Option<OpenMenu>,
    pub menu_gen: usize,
    pub dialog: Option<ConfirmDialog>,
    /// History tab: commit menu, dialogs, amend mode (features/history).
    pub history_ui: HistoryUi,
    /// Stash dialogs (create / rename / drop / switch branch) and their input.
    pub stash_ui: StashUi,
    /// Merge dialog, branch/conflict menus and branch dialogs (features/merge).
    pub merge_ui: MergeUi,
    /// Branch context menu and rename / delete / create dialogs (features/branches).
    pub branch_ui: BranchUi,
    /// Window corner radius for this frame (0 unless floating under CSD);
    /// layers touching a window corner round their own background with it.
    pub corner_radius: f32,
    /// Error shown when no repository is open (e.g. bad path on startup).
    pub startup_error: Option<String>,

    pub summary: Entity<EditableTextState>,
    pub description: Entity<EditableTextState>,
    pub filter: Entity<EditableTextState>,
    pub name_input: Entity<EditableTextState>,
    pub url_input: Entity<EditableTextState>,

    /// List that ↑/↓ act on: the one last clicked (None = the tab's main list).
    pub nav_list: Option<super::navigation::NavList>,
    pub changes_scroll: UniformListScrollHandle,
    pub history_scroll: UniformListScrollHandle,
    pub diff_scroll: UniformListScrollHandle,
    /// Identity of the diff shown last; scroll resets when it changes.
    pub diff_key: Option<SharedString>,
    /// Diff options popover + split-row cache (see `features/diff`).
    pub diff_view: DiffView,
    pub diff_select: DiffSelectUi,
    pub commit_files_scroll: UniformListScrollHandle,
    /// Height of the selected commit's message area (history header).
    pub commit_msg: crate::features::history::MessageResize,
    pub popover_scroll: UniformListScrollHandle,

    pub(super) focus: FocusHandle,
    /// Auto-dismiss timer of the current success notice (features/notices).
    pub(crate) notice_task: Option<Task<()>>,
    pub(super) repo_sub: Option<Subscription>,
    pub(super) repo_event_sub: Option<Subscription>,
    pub(super) _subs: Vec<Subscription>,
}
