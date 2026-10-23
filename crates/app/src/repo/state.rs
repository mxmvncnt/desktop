use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use gd_git::{Branch, Commit, Diff, DiffOptions, FileChange, Repository, WorkingStatus};
use gpui::{Context, Task};

use crate::features::diff::{DiffHighlight, HighlightCache};
use crate::watcher::RepoWatcher;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Changes,
    History,
}

/// Network/long-running operation in flight (drives toolbar busy state).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RemoteOp {
    Fetch,
    Pull,
    Push,
}

/// How much a refresh reloads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RefreshScope {
    /// Worktree status only (files edited).
    Status,
    /// Also branches, stashes, push state and history (git metadata changed).
    Full,
}

impl RefreshScope {
    pub fn from_metadata_changed(changed: bool) -> Self {
        if changed { Self::Full } else { Self::Status }
    }

    /// The wider of the two.
    pub fn or(self, other: Self) -> Self {
        if self == Self::Full || other == Self::Full { Self::Full } else { Self::Status }
    }
}

/// The commit made in this session, offered for undo (Desktop's strip above
/// the commit box). Hidden after a branch switch or an undo.
#[derive(Clone, Debug)]
pub struct LastCommit {
    pub summary: String,
    pub description: String,
    pub branch: String,
    /// Unix seconds.
    pub time: i64,
}

/// Emitted for the workspace, which owns the commit text inputs.
pub enum RepoEvent {
    /// The last commit was undone: put the message back in the commit box.
    CommitUndone { summary: String, description: String },
    /// Checkout requested while the working tree has changes: the workspace
    /// asks the user whether to stash them or bring them along.
    SwitchBranchWithChanges(Branch),
    /// A merge-like operation paused; git prepared this commit message.
    PreparedMessage { summary: String, description: String },
}

impl gpui::EventEmitter<RepoEvent> for RepoModel {}

pub struct RepoModel {
    pub repo: Repository,
    pub tab: Tab,
    /// App-wide diff options (set by the Workspace); changing them reloads the visible diffs.
    pub diff_options: DiffOptions,

    // ---- working directory ----
    pub status: Option<WorkingStatus>,
    /// Paths excluded from the next commit (Desktop's unchecked boxes). We
    /// store the *excluded* set so new files default to included.
    pub excluded: HashSet<String>,
    /// Files with only some lines selected (see `line_selection.rs`).
    pub partial: HashMap<String, crate::features::diff::LineSelection>,
    pub selected_change: Option<usize>,
    pub change_diff: Option<Arc<Diff>>,
    pub(super) change_diff_gen: u64,
    /// What `change_diff` was loaded for, so a reload of the same unchanged
    /// file keeps the (possibly expanded) diff on screen.
    pub(super) change_diff_key: Option<(String, DiffOptions)>,
    pub(super) _change_diff_task: Option<Task<()>>,

    // ---- refresh coalescing / live updates ----
    /// A refresh is running on the background executor.
    pub(super) refreshing: bool,
    /// A trigger arrived while refreshing: run exactly one more afterwards.
    pub(super) pending_refresh: Option<RefreshScope>,
    /// Number of refreshes started (debug counter; tests assert idle = stable).
    pub refresh_count: u64,
    /// When the file watcher last triggered a refresh (focus refresh skips if recent).
    pub last_watch_refresh: Option<Instant>,
    /// mtime of `.git/FETCH_HEAD`.
    pub last_fetch: Option<SystemTime>,
    pub(super) last_background_fetch: Option<Instant>,
    pub(super) _watcher: Option<RepoWatcher>,
    pub(super) _watch_task: Option<Task<()>>,
    pub(super) _background_fetch_task: Option<Task<()>>,
    /// Syntax colors for `change_diff`; `None` until the background pass lands.
    pub change_hl: Option<Arc<DiffHighlight>>,
    pub(super) _change_hl_task: Option<Task<()>>,

    // ---- history ----
    pub commits: Vec<Commit>,
    pub history_exhausted: bool,
    pub(super) history_loading: bool,
    pub selected_commit: Option<usize>,
    pub commit_files: Vec<FileChange>,
    pub selected_commit_file: Option<usize>,
    pub commit_diff: Option<Arc<Diff>>,
    pub commit_hl: Option<Arc<DiffHighlight>>,
    pub(super) commit_gen: u64,
    pub(super) _commit_task: Option<Task<()>>,
    pub(super) _commit_hl_task: Option<Task<()>>,
    pub(super) hl_cache: HighlightCache,
    /// Push status, tags, multi-select (see `features/history`).
    pub history: crate::features::history::HistoryState,

    // ---- branches / remote ----
    pub branches: Vec<Branch>,
    pub remote_op: Option<RemoteOp>,
    pub committing: bool,
    pub last_commit: Option<LastCommit>,
    /// Last error from any operation, shown as a dismissible banner.
    pub error: Option<String>,
    /// Transient success message ("Committed 3 files").
    pub notice: Option<String>,

    /// Stash list, selection, and panel state (see features/stashes).
    pub stashes: crate::features::stashes::StashState,
    /// Pull-request preview (see `features/pr_preview`).
    pub pr: crate::features::pr_preview::PrPreviewState,
    /// Merge / rebase / conflict state (see `features/merge/`).
    pub merge: crate::features::merge::MergeState,
}

impl RepoModel {
    pub fn new(repo: Repository, cx: &mut Context<Self>) -> Self {
        let mut this = Self::empty(repo);
        this.refresh(cx);
        this.start_watcher(cx);
        this.start_background_fetch(cx);
        this
    }

    fn empty(repo: Repository) -> Self {
        Self {
            repo,
            tab: Tab::Changes,
            diff_options: DiffOptions::default(),
            status: None,
            excluded: HashSet::new(),
            partial: HashMap::new(),
            selected_change: None,
            change_diff: None,
            change_diff_gen: 0,
            change_diff_key: None,
            _change_diff_task: None,
            refreshing: false,
            pending_refresh: None,
            refresh_count: 0,
            last_watch_refresh: None,
            last_fetch: None,
            last_background_fetch: None,
            _watcher: None,
            _watch_task: None,
            _background_fetch_task: None,
            change_hl: None,
            _change_hl_task: None,
            commits: Vec::new(),
            history_exhausted: false,
            history_loading: false,
            selected_commit: None,
            commit_files: Vec::new(),
            selected_commit_file: None,
            commit_diff: None,
            commit_hl: None,
            commit_gen: 0,
            _commit_task: None,
            _commit_hl_task: None,
            hl_cache: HighlightCache::default(),
            history: Default::default(),
            branches: Vec::new(),
            remote_op: None,
            committing: false,
            last_commit: None,
            error: None,
            notice: None,
            stashes: Default::default(),
            pr: Default::default(),
            merge: Default::default(),
        }
    }

    pub fn branch_name(&self) -> String {
        match &self.status {
            Some(s) => s.branch.clone().unwrap_or_else(|| "(detached HEAD)".into()),
            None => "…".into(),
        }
    }

    /// The undoable commit, only while still on the branch it was made on.
    pub fn undoable_commit(&self) -> Option<&LastCommit> {
        self.last_commit
            .as_ref()
            .filter(|c| self.status.as_ref().and_then(|s| s.branch.as_deref()) == Some(c.branch.as_str()))
    }

    pub fn changes(&self) -> &[FileChange] {
        self.status.as_ref().map(|s| s.files.as_slice()).unwrap_or(&[])
    }
}
