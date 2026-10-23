//! History state: the model side ([`HistoryState`], a field of `RepoModel`)
//! and the Workspace side ([`HistoryUi`]: menu, dialog, amend mode, inputs).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use gd_git::{Commit, ResetMode};
use gpui::{AppContext, Context, Entity, Pixels, Point, Subscription, Task};
use gpui_elements::editable_text::{EditableTextState, StringStorage, TextChanged};

use crate::workspace::Workspace;

/// Everything the History tab needs beyond the commit list itself.
#[derive(Default)]
pub struct HistoryState {
    /// Commits reachable from HEAD that are not on the remote yet.
    pub unpushed: HashSet<String>,
    /// The repository has no remote at all: every commit is local, so
    /// `unpushed` (empty in that case) must not be read as "all pushed".
    pub all_local: bool,
    /// Whether `unpushed` has been computed at least once.
    pub push_state_loaded: bool,
    /// Tags by commit sha.
    pub tags: HashMap<String, Vec<String>>,
    /// `https://github.com/owner/repo` when the remote is GitHub.
    pub web_url: Option<String>,
    /// Multi-selection (shas). Empty means "just `selected_commit`".
    pub multi: HashSet<String>,
    pub(super) load_gen: u64,
    pub(super) task: Option<Task<()>>,
}

/// How a click on a history row changes the selection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClickKind {
    /// Select just this commit.
    Plain,
    /// Ctrl/Cmd: toggle this commit in the multi-selection.
    Toggle,
    /// Shift: range from the primary selection.
    Range,
}

/// Everything the History feature adds to `Workspace`. A plain struct (not an
/// `Entity`): Esc/Enter/arrow routing in workspace.rs reads `menu`/`dialog`.
pub struct HistoryUi {
    pub menu: Option<HistoryMenu>,
    pub menu_gen: usize,
    pub dialog: Option<HistoryDialog>,
    /// Commit box is amending the last commit of this repository.
    pub amend: Option<AmendState>,
    /// Inputs shared by the dialogs (squash message, branch / tag name).
    pub msg_summary: Entity<EditableTextState>,
    pub msg_desc: Entity<EditableTextState>,
    pub name: Entity<EditableTextState>,
    _subs: Vec<Subscription>,
}

pub struct AmendState {
    pub root: PathBuf,
}

impl HistoryUi {
    pub fn new(cx: &mut Context<Workspace>) -> Self {
        let mk = |cx: &mut Context<Workspace>| cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        let (msg_summary, msg_desc, name) = (mk(cx), mk(cx), mk(cx));
        let subs = [&msg_summary, &msg_desc, &name]
            .into_iter()
            .map(|i| cx.subscribe(i, |_: &mut Workspace, _, _: &TextChanged, cx| cx.notify()))
            .collect();
        Self { menu: None, menu_gen: 0, dialog: None, amend: None, msg_summary, msg_desc, name, _subs: subs }
    }

    pub fn is_open(&self) -> bool {
        self.menu.is_some() || self.dialog.is_some()
    }
}

pub struct HistoryMenu {
    pub position: Point<Pixels>,
    pub selected: Option<usize>,
    pub ctx: MenuCtx,
}

/// Snapshot of everything the menu rows depend on, taken at open time.
pub struct MenuCtx {
    pub commit: Commit,
    /// Multi-selection, newest first (empty unless 2+ are selected).
    pub multi: Vec<Commit>,
    pub is_head: bool,
    pub unpushed: bool,
    pub tags: Vec<String>,
    pub web_url: Option<String>,
    pub blocked: bool,
    pub squashable: bool,
    pub can_up: bool,
    pub can_down: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum HistoryAction {
    Amend,
    Undo,
    Reset,
    Checkout,
    MoveUp,
    MoveDown,
    Revert,
    CreateBranch,
    CreateTag,
    DeleteTag(String),
    CherryPick,
    Squash,
    CopySha,
    CopyTags,
    ViewOnGitHub,
}

impl HistoryAction {
    /// Rewrites history, so it warns first when the commit is already pushed.
    pub fn rewrites_history(&self) -> bool {
        matches!(self, Self::Undo | Self::Reset | Self::MoveUp | Self::MoveDown)
    }
}

/// Branch choice of the cherry-pick dialog.
#[derive(Clone)]
pub struct CherryPickTarget {
    pub branches: Vec<String>,
    pub current: Option<String>,
    pub selected: usize,
}

#[derive(Clone)]
pub enum HistoryDialog {
    Reset { commit: Commit, mode: ResetMode },
    Checkout(Commit),
    AmendPushed(Commit),
    /// Undo / reset / move on a commit that's already on the remote: allowed,
    /// but warn that the branch will need a force push. Confirm runs `action`.
    PushedRewrite { action: HistoryAction, commit: Commit },
    CreateBranch(Commit),
    CreateTag(Commit),
    DeleteTag(String),
    CherryPick { commits: Vec<Commit>, target: CherryPickTarget },
    Squash { commits: Vec<Commit>, any_pushed: bool },
}
