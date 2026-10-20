//! `RepoModel` — all state for the currently-open repository, plus the async
//! plumbing that runs git on the background executor.
//!
//! Rules (see docs/ARCHITECTURE.md §Threading):
//! - Never call `gd_git` on the UI thread. Use [`RepoModel::run`] /
//!   `cx.background_spawn` and apply results in `this.update(..)`.
//! - Stale results are dropped: each load bumps a generation counter and the
//!   completion handler ignores results from an older generation (the user
//!   may have clicked another file while a big diff was loading).

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use crate::diff_highlight::{DiffHighlight, HighlightCache, highlight_diff};
use gd_git::{Branch, Commit, Diff, DiffOptions, FileChange, Repository, WorkingStatus};
use gpui::{AppContext, Context, Task};

use crate::watcher::RepoWatcher;

/// Background fetch cadence (Desktop: every 1h; the task asks for 30 min).
pub const BACKGROUND_FETCH_INTERVAL: Duration = Duration::from_secs(30 * 60);
const BACKGROUND_FETCH_FIRST_CHECK: Duration = Duration::from_secs(10);
/// How often the background task wakes up (also re-renders "Last fetched").
const BACKGROUND_TICK: Duration = Duration::from_secs(60);

/// History is paged in so huge repos open instantly.
pub const HISTORY_PAGE: usize = 200;

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
    pub selected_change: Option<usize>,
    pub change_diff: Option<Arc<Diff>>,
    change_diff_gen: u64,
    _change_diff_task: Option<Task<()>>,

    // ---- refresh coalescing / live updates ----
    /// A refresh is running on the background executor.
    refreshing: bool,
    /// A trigger arrived while refreshing: run exactly one more afterwards.
    /// `true` means "full" (branches + history too).
    pending_refresh: Option<bool>,
    /// Number of refreshes started (debug counter; tests assert idle = stable).
    pub refresh_count: u64,
    /// When the file watcher last triggered a refresh (focus refresh skips if recent).
    pub last_watch_refresh: Option<Instant>,
    /// mtime of `.git/FETCH_HEAD`.
    pub last_fetch: Option<SystemTime>,
    last_background_fetch: Option<Instant>,
    _watcher: Option<RepoWatcher>,
    _watch_task: Option<Task<()>>,
    _background_fetch_task: Option<Task<()>>,
    /// Syntax colors for `change_diff`; `None` until the background pass lands.
    pub change_hl: Option<Arc<DiffHighlight>>,
    _change_hl_task: Option<Task<()>>,

    // ---- history ----
    pub commits: Vec<Commit>,
    pub history_exhausted: bool,
    history_loading: bool,
    pub selected_commit: Option<usize>,
    pub commit_files: Vec<FileChange>,
    pub selected_commit_file: Option<usize>,
    pub commit_diff: Option<Arc<Diff>>,
    pub commit_hl: Option<Arc<DiffHighlight>>,
    commit_gen: u64,
    _commit_task: Option<Task<()>>,
    _commit_hl_task: Option<Task<()>>,
    hl_cache: HighlightCache,

    // ---- branches / remote ----
    pub branches: Vec<Branch>,
    pub remote_op: Option<RemoteOp>,
    pub committing: bool,
    pub last_commit: Option<LastCommit>,
    /// Last error from any operation, shown as a dismissible banner.
    pub error: Option<String>,
    /// Transient success message ("Committed 3 files").
    pub notice: Option<String>,

    /// Stash list, selection, and panel state (see stashes.rs).
    pub stashes: crate::stashes::StashState,
    /// Merge / rebase / conflict state (see `merge_ops.rs`).
    pub merge: crate::merge_ops::MergeState,
}

impl RepoModel {
    pub fn new(repo: Repository, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            repo,
            tab: Tab::Changes,
            diff_options: DiffOptions::default(),
            status: None,
            excluded: HashSet::new(),
            selected_change: None,
            change_diff: None,
            change_diff_gen: 0,
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
            branches: Vec::new(),
            remote_op: None,
            committing: false,
            last_commit: None,
            error: None,
            notice: None,
            stashes: Default::default(),
            merge: Default::default(),
        };
        this.refresh(cx);
        this.start_watcher(cx);
        this.start_background_fetch(cx);
        this
    }

    /// Watch the working tree + git metadata; each debounced burst requests a
    /// (coalesced) refresh. The watcher thread dies with this model.
    fn start_watcher(&mut self, cx: &mut Context<Self>) {
        let (watcher, rx) = RepoWatcher::start(&self.repo);
        self._watcher = Some(watcher);
        self._watch_task = Some(cx.spawn(async move |this, cx| {
            while let Ok(mut full) = rx.recv().await {
                while let Ok(more) = rx.try_recv() {
                    full |= more;
                }
                let alive = this
                    .update(cx, |this, cx| {
                        this.last_watch_refresh = Some(Instant::now());
                        this.request_refresh(full, cx);
                    })
                    .is_ok();
                if !alive {
                    break;
                }
            }
        }));
    }

    /// Fetch every 30 minutes, first check shortly after opening. Errors are
    /// only logged. Wakes once a minute (also to refresh "Last fetched Nm ago").
    fn start_background_fetch(&mut self, cx: &mut Context<Self>) {
        self._background_fetch_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(BACKGROUND_FETCH_FIRST_CHECK).await;
            loop {
                if this
                    .update(cx, |this, cx| {
                        this.maybe_background_fetch(cx);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
                cx.background_executor().timer(BACKGROUND_TICK).await;
            }
        }));
    }

    fn maybe_background_fetch(&mut self, cx: &mut Context<Self>) {
        if self.remote_op.is_some() {
            return;
        }
        let due = |t: Option<Duration>| t.is_none_or(|d| d >= BACKGROUND_FETCH_INTERVAL);
        let since_fetch = self.last_fetch.and_then(|t| t.elapsed().ok());
        let since_attempt = self.last_background_fetch.map(|t| t.elapsed());
        if !due(since_fetch) || !due(since_attempt) {
            return;
        }
        self.last_background_fetch = Some(Instant::now());
        let repo = self.repo.clone();
        cx.spawn(async move |this, cx| {
            let has_remote = cx
                .background_spawn(async move { repo.remotes().map(|r| !r.is_empty()).unwrap_or(false) })
                .await;
            if !has_remote {
                return;
            }
            this.update(cx, |this, cx| {
                // Another remote op may have started meanwhile.
                if this.remote_op.is_some() {
                    return;
                }
                log::info!("background fetch: {}", this.repo.name());
                this.remote_op = Some(RemoteOp::Fetch);
                cx.notify();
                let repo = this.repo.clone();
                cx.spawn(async move |this, cx| {
                    let res = cx.background_spawn(async move { repo.fetch() }).await;
                    this.update(cx, |this, cx| {
                        this.remote_op = None;
                        if let Err(e) = res {
                            log::warn!("background fetch failed: {e}");
                        }
                        this.refresh(cx);
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            })
            .ok();
        })
        .detach();
    }

    pub fn branch_name(&self) -> String {
        match &self.status {
            Some(s) => s
                .branch
                .clone()
                .unwrap_or_else(|| "(detached HEAD)".into()),
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

    pub fn included_paths(&self) -> Vec<String> {
        self.changes()
            .iter()
            .filter(|c| !self.excluded.contains(&c.path))
            .map(|c| c.path.clone())
            .collect()
    }

    /// Reload status + branches (+ history if it was loaded). Cheap enough to
    /// call on window focus and after every operation.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.request_refresh(true, cx);
    }

    /// Coalescing refresh: at most one runs at a time; triggers arriving
    /// meanwhile collapse into exactly one follow-up run. `full` also reloads
    /// branches and history (git metadata changed); otherwise only status.
    pub fn request_refresh(&mut self, full: bool, cx: &mut Context<Self>) {
        if self.refreshing {
            self.pending_refresh = Some(self.pending_refresh.unwrap_or(false) || full);
            return;
        }
        self.refreshing = true;
        self.refresh_count += 1;
        log::debug!(
            "refresh #{} ({}) {}",
            self.refresh_count,
            if full { "full" } else { "status" },
            self.repo.name()
        );
        let repo = self.repo.clone();
        cx.spawn(async move |this, cx| {
            let (status, branches, last_fetch, progress) = cx
                .background_spawn(async move {
                    let status = repo.status();
                    let branches = full.then(|| repo.branches());
                    let progress = repo.in_progress().map(|p| {
                        let msg = p.commit_finishes().then(|| repo.prepared_message()).flatten();
                        (p, msg)
                    });
                    (status, branches, repo.last_fetch_time(), progress)
                })
                .await;
            this.update(cx, |this, cx| {
                this.refreshing = false;
                match status {
                    Ok(status) => this.apply_status(status, cx),
                    Err(e) => this.error = Some(e.to_string()),
                }
                if let Some(Ok(b)) = branches {
                    this.branches = b;
                }
                this.last_fetch = last_fetch;
                if full {
                    this.reload_stashes(cx);
                }
                this.apply_progress(progress, cx);
                if full && (!this.commits.is_empty() || this.tab == Tab::History) {
                    this.reload_history(cx);
                }
                cx.notify();
                if let Some(full) = this.pending_refresh.take() {
                    this.request_refresh(full, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// Store the paused operation; announce git's prepared message when a
    /// different operation newly paused (the workspace fills the commit box).
    fn apply_progress(
        &mut self,
        progress: Option<(gd_git::InProgress, Option<(String, String)>)>,
        cx: &mut Context<Self>,
    ) {
        let (new, msg) = match progress {
            Some((p, m)) => (Some(p), m),
            None => (None, None),
        };
        let newly = new.is_some()
            && self.merge.in_progress.as_ref().map(std::mem::discriminant) != new.as_ref().map(std::mem::discriminant);
        self.merge.in_progress = new;
        if let (true, Some((summary, description))) = (newly, msg) {
            cx.emit(crate::merge_ops::message_event(summary, description));
        }
    }

    fn apply_status(&mut self, status: WorkingStatus, cx: &mut Context<Self>) {
        // Keep the selection on the same path across refreshes.
        let selected_path = self
            .selected_change
            .and_then(|ix| self.changes().get(ix))
            .map(|c| c.path.clone());
        let paths: HashSet<&str> = status.files.iter().map(|f| f.path.as_str()).collect();
        self.excluded.retain(|p| paths.contains(p.as_str()));
        let changed = self.status.as_ref().map(|s| &s.files) != Some(&status.files);
        self.status = Some(status);
        self.selected_change = selected_path
            .and_then(|p| self.changes().iter().position(|c| c.path == p))
            .or(if self.changes().is_empty() { None } else { Some(0) });
        if changed || self.change_diff.is_none() {
            self.load_change_diff(cx);
        }
    }

    pub fn select_change(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.stashes.viewing {
            // Back from the stash view to the working diff.
            self.stashes.viewing = false;
            cx.notify();
        }
        if self.selected_change == Some(ix) {
            return;
        }
        self.selected_change = Some(ix);
        self.load_change_diff(cx);
        cx.notify();
    }

    fn load_change_diff(&mut self, cx: &mut Context<Self>) {
        self.change_diff_gen += 1;
        let generation = self.change_diff_gen;
        let Some(change) = self
            .selected_change
            .and_then(|ix| self.changes().get(ix))
            .cloned()
        else {
            self.change_diff = None;
            self.change_hl = None;
            self._change_hl_task = None;
            return;
        };
        let repo = self.repo.clone();
        let path = change.path.clone();
        let opts = self.diff_options;
        self._change_diff_task = Some(cx.spawn(async move |this, cx| {
            let diff = cx
                .background_spawn(async move { repo.working_diff(&change, opts) })
                .await;
            this.update(cx, |this, cx| {
                if this.change_diff_gen != generation {
                    return;
                }
                match diff {
                    Ok(d) => {
                        let d = Arc::new(d);
                        this.change_diff = Some(d.clone());
                        this.change_hl = None;
                        this._change_hl_task = Some(this.spawn_highlight(d, path, false, cx));
                    }
                    Err(e) => this.error = Some(e.to_string()),
                }
                cx.notify();
            })
            .ok();
        }));
    }

    pub fn toggle_included(&mut self, path: &str, cx: &mut Context<Self>) {
        if !self.excluded.remove(path) {
            self.excluded.insert(path.to_string());
        }
        cx.notify();
    }

    pub fn set_all_included(&mut self, included: bool, cx: &mut Context<Self>) {
        if included {
            self.excluded.clear();
        } else {
            self.excluded = self.changes().iter().map(|c| c.path.clone()).collect();
        }
        cx.notify();
    }

    pub fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        if tab == Tab::History && self.commits.is_empty() {
            self.reload_history(cx);
        }
        cx.notify();
    }

    // ---- history -------------------------------------------------------

    fn reload_history(&mut self, cx: &mut Context<Self>) {
        let repo = self.repo.clone();
        let count = self.commits.len().max(HISTORY_PAGE);
        self.history_loading = true;
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move { repo.log(0, count) })
                .await;
            this.update(cx, |this, cx| {
                this.history_loading = false;
                match res {
                    Ok(commits) => {
                        let selected_sha = this
                            .selected_commit
                            .and_then(|ix| this.commits.get(ix))
                            .map(|c| c.sha.clone());
                        this.history_exhausted = commits.len() < count;
                        this.commits = commits;
                        let ix = selected_sha
                            .and_then(|s| this.commits.iter().position(|c| c.sha == s))
                            .or(if this.commits.is_empty() { None } else { Some(0) });
                        if ix != this.selected_commit || this.commit_files.is_empty() {
                            this.selected_commit = None;
                            if let Some(ix) = ix {
                                this.select_commit(ix, cx);
                            } else {
                                this.commit_files.clear();
                                this.commit_diff = None;
                                this.commit_hl = None;
                            }
                        }
                    }
                    Err(e) => this.error = Some(e.to_string()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Called by the history list when the user scrolls near the end.
    pub fn load_more_history(&mut self, cx: &mut Context<Self>) {
        if self.history_loading || self.history_exhausted {
            return;
        }
        self.history_loading = true;
        let repo = self.repo.clone();
        let skip = self.commits.len();
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move { repo.log(skip, HISTORY_PAGE) })
                .await;
            this.update(cx, |this, cx| {
                this.history_loading = false;
                if let Ok(mut more) = res {
                    this.history_exhausted = more.len() < HISTORY_PAGE;
                    if this.commits.len() == skip {
                        this.commits.append(&mut more);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn select_commit(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.selected_commit == Some(ix) {
            return;
        }
        let Some(commit) = self.commits.get(ix) else {
            return;
        };
        self.selected_commit = Some(ix);
        self.commit_gen += 1;
        let generation = self.commit_gen;
        let sha = commit.sha.clone();
        let repo = self.repo.clone();
        let opts = self.diff_options;
        self._commit_task = Some(cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move {
                    let files = repo.commit_files(&sha)?;
                    let diff = match files.first() {
                        Some(f) => Some((repo.commit_diff(&sha, f, opts)?, f.path.clone())),
                        None => None,
                    };
                    Ok::<_, gd_git::GitError>((files, diff))
                })
                .await;
            this.update(cx, |this, cx| {
                if this.commit_gen != generation {
                    return;
                }
                match res {
                    Ok((files, diff)) => {
                        this.selected_commit_file = (!files.is_empty()).then_some(0);
                        this.commit_files = files;
                        this.set_commit_diff(diff, cx);
                    }
                    Err(e) => this.error = Some(e.to_string()),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn set_commit_diff(&mut self, diff: Option<(Diff, String)>, cx: &mut Context<Self>) {
        self.commit_hl = None;
        self._commit_hl_task = None;
        match diff {
            Some((d, path)) => {
                let d = Arc::new(d);
                self.commit_diff = Some(d.clone());
                self._commit_hl_task = Some(self.spawn_highlight(d, path, true, cx));
            }
            None => self.commit_diff = None,
        }
    }

    /// Highlight `diff` on the background executor. The result is applied only
    /// if `diff` is still the displayed one (`Arc::ptr_eq`); a newer diff
    /// replaces the stored task, which cancels this one if it hasn't started.
    fn spawn_highlight(
        &self,
        diff: Arc<Diff>,
        path: String,
        commit: bool,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        let cache = self.hl_cache.clone();
        let for_task = diff.clone();
        cx.spawn(async move |this, cx| {
            let hl = cx
                .background_spawn(async move { highlight_diff(&for_task, &path, &cache) })
                .await;
            this.update(cx, |this, cx| {
                let (slot, current) = if commit {
                    (&mut this.commit_hl, &this.commit_diff)
                } else {
                    (&mut this.change_hl, &this.change_diff)
                };
                if hl.is_some() && current.as_ref().is_some_and(|c| Arc::ptr_eq(c, &diff)) {
                    *slot = hl;
                    cx.notify();
                }
            })
            .ok();
        })
    }

    /// Change how diffs are produced and reload the visible ones. Uses the
    /// same generation counters as selection changes, so stale results drop.
    pub fn set_diff_options(&mut self, opts: DiffOptions, cx: &mut Context<Self>) {
        if self.diff_options == opts {
            return;
        }
        self.diff_options = opts;
        self.load_change_diff(cx);
        if let Some(ix) = self.selected_commit_file {
            self.select_commit_file(ix, cx);
        }
        cx.notify();
    }

    pub fn select_commit_file(&mut self, ix: usize, cx: &mut Context<Self>) {
        let (Some(commit), Some(file)) = (
            self.selected_commit.and_then(|c| self.commits.get(c)),
            self.commit_files.get(ix),
        ) else {
            return;
        };
        self.selected_commit_file = Some(ix);
        self.commit_gen += 1;
        let generation = self.commit_gen;
        let (sha, file, repo) = (commit.sha.clone(), file.clone(), self.repo.clone());
        let opts = self.diff_options;
        self._commit_task = Some(cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move {
                    let path = file.path.clone();
                    repo.commit_diff(&sha, &file, opts).map(|d| (d, path))
                })
                .await;
            this.update(cx, |this, cx| {
                if this.commit_gen != generation {
                    return;
                }
                match res {
                    Ok(d) => this.set_commit_diff(Some(d), cx),
                    Err(e) => this.error = Some(e.to_string()),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    // ---- write operations ----------------------------------------------

    /// Run a blocking git operation off-thread, then refresh. `on_ok` runs on
    /// the UI thread after success.
    pub fn run<R: Send + 'static>(
        &mut self,
        op: impl FnOnce(&Repository) -> gd_git::Result<R> + Send + 'static,
        on_ok: impl FnOnce(&mut Self, R, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let repo = self.repo.clone();
        self.error = None;
        cx.spawn(async move |this, cx| {
            let res = cx.background_spawn(async move { op(&repo) }).await;
            this.update(cx, |this, cx| {
                this.remote_op = None;
                this.committing = false;
                match res {
                    Ok(r) => on_ok(this, r, cx),
                    Err(e) => this.error = Some(e.to_string()),
                }
                this.refresh(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn commit(&mut self, summary: String, description: String, cx: &mut Context<Self>) {
        let paths = self.included_paths();
        let n = paths.len();
        let branch = self.branch_name();
        self.committing = true;
        cx.notify();
        if self.merge.in_progress.as_ref().is_some_and(|p| p.commit_finishes()) {
            return self.commit_merge(summary, description, cx);
        }
        let message = (summary.clone(), description.clone());
        self.run(
            move |r| r.commit(&summary, &description, &paths),
            move |this, _sha, _| {
                this.last_commit = Some(LastCommit {
                    summary: message.0,
                    description: message.1,
                    branch,
                    time: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0),
                });
                this.notice = Some(format!(
                    "Committed {n} file{}",
                    if n == 1 { "" } else { "s" }
                ));
                this.selected_change = None;
            },
            cx,
        );
    }

    pub fn remote(&mut self, op: RemoteOp, cx: &mut Context<Self>) {
        if self.remote_op.is_some() {
            return;
        }
        self.remote_op = Some(op);
        cx.notify();
        let (branch, has_upstream) = match &self.status {
            Some(s) => (s.branch.clone().unwrap_or_default(), s.upstream.is_some()),
            None => (String::new(), false),
        };
        self.run(
            move |r| match op {
                RemoteOp::Fetch => r.fetch(),
                RemoteOp::Pull => r.pull(),
                RemoteOp::Push => r.push(&branch, has_upstream),
            },
            move |this, _, _| {
                this.notice = Some(
                    match op {
                        RemoteOp::Fetch => "Fetched from remote",
                        RemoteOp::Pull => "Pulled from remote",
                        RemoteOp::Push => "Pushed to remote",
                    }
                    .into(),
                )
            },
            cx,
        );
    }

    /// Switch branch. With uncommitted changes the workspace first asks
    /// whether to stash them (Desktop's dialog).
    pub fn checkout(&mut self, branch: Branch, cx: &mut Context<Self>) {
        if !self.changes().is_empty() {
            cx.emit(RepoEvent::SwitchBranchWithChanges(branch));
            return;
        }
        self.checkout_now(branch, cx);
    }

    pub fn checkout_now(&mut self, branch: Branch, cx: &mut Context<Self>) {
        self.run(
            move |r| r.checkout(&branch),
            |this, _, _| {
                this.commits.clear();
                this.selected_commit = None;
                this.last_commit = None;
            },
            cx,
        );
    }

    pub fn create_branch(&mut self, name: String, cx: &mut Context<Self>) {
        self.run(move |r| r.create_branch(&name), |_, _, _| {}, cx);
    }

    /// Append gitignore `patterns` to `.gitignore`, then refresh.
    pub fn ignore(&mut self, patterns: Vec<String>, notice: String, cx: &mut Context<Self>) {
        self.run(
            move |r| r.ignore(&patterns),
            move |this, _, _| this.notice = Some(notice),
            cx,
        );
    }

    pub fn discard(&mut self, changes: Vec<FileChange>, cx: &mut Context<Self>) {
        self.run(move |r| r.discard(&changes), |_, _, _| {}, cx);
    }

    /// Soft-reset HEAD~1 and hand the message back via [`RepoEvent::CommitUndone`].
    pub fn undo_last_commit(&mut self, cx: &mut Context<Self>) {
        let last = self.last_commit.take();
        self.run(
            |r| r.undo_last_commit(),
            move |this, _, cx| {
                this.commits.clear();
                this.selected_commit = None;
                if let Some(c) = last {
                    cx.emit(RepoEvent::CommitUndone {
                        summary: c.summary,
                        description: c.description,
                    });
                }
            },
            cx,
        );
    }
}
