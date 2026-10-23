//! Coalesced refresh of status / branches / history, and the file watcher
//! that triggers it.

use std::collections::HashSet;
use std::time::{Instant, SystemTime};

use gd_git::{Branch, InProgress, Repository, WorkingStatus};
use gpui::{AppContext, Context};

use super::state::{RefreshScope, RepoModel, Tab};
use crate::watcher::RepoWatcher;

/// Everything one refresh reads from git (gathered off the UI thread).
struct RefreshData {
    status: gd_git::Result<WorkingStatus>,
    /// Only read for `RefreshScope::Full`.
    branches: Option<gd_git::Result<Vec<Branch>>>,
    last_fetch: Option<SystemTime>,
    paused: Option<PausedOp>,
}

/// A merge-like operation stopped in the middle, with git's prepared message
/// when finishing it creates a commit.
struct PausedOp {
    op: InProgress,
    message: Option<(String, String)>,
}

impl RefreshData {
    fn read(repo: &Repository, scope: RefreshScope) -> Self {
        Self {
            status: repo.status(),
            branches: (scope == RefreshScope::Full).then(|| repo.branches()),
            paused: repo.in_progress().map(|op| {
                let message = op.commit_finishes().then(|| repo.prepared_message()).flatten();
                PausedOp { op, message }
            }),
            last_fetch: repo.last_fetch_time(),
        }
    }
}

impl RepoModel {
    /// Watch the working tree + git metadata; each debounced burst requests a
    /// (coalesced) refresh. The watcher thread dies with this model.
    pub(super) fn start_watcher(&mut self, cx: &mut Context<Self>) {
        let (watcher, rx) = RepoWatcher::start(&self.repo);
        self._watcher = Some(watcher);
        self._watch_task = Some(cx.spawn(async move |this, cx| {
            while let Ok(mut metadata_changed) = rx.recv().await {
                while let Ok(more) = rx.try_recv() {
                    metadata_changed |= more;
                }
                let scope = RefreshScope::from_metadata_changed(metadata_changed);
                let alive = this
                    .update(cx, |this, cx| {
                        this.last_watch_refresh = Some(Instant::now());
                        this.request_refresh(scope, cx);
                    })
                    .is_ok();
                if !alive {
                    break;
                }
            }
        }));
    }

    /// Reload status + branches (+ history if it was loaded). Cheap enough to
    /// call on window focus and after every operation.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.request_refresh(RefreshScope::Full, cx);
    }

    /// Coalescing refresh: at most one runs at a time; triggers arriving
    /// meanwhile collapse into exactly one follow-up run. `Full` also reloads
    /// branches and history (git metadata changed); otherwise only status.
    pub fn request_refresh(&mut self, scope: RefreshScope, cx: &mut Context<Self>) {
        if self.refreshing {
            self.pending_refresh = Some(self.pending_refresh.map_or(scope, |p| p.or(scope)));
            return;
        }
        self.refreshing = true;
        self.refresh_count += 1;
        log::debug!("refresh #{} ({:?}) {}", self.refresh_count, scope, self.repo.name());
        let repo = self.repo.clone();
        cx.spawn(async move |this, cx| {
            let data = cx.background_spawn(async move { RefreshData::read(&repo, scope) }).await;
            this.update(cx, |this, cx| this.finish_refresh(data, scope, cx)).ok();
        })
        .detach();
    }

    fn finish_refresh(&mut self, data: RefreshData, scope: RefreshScope, cx: &mut Context<Self>) {
        let full = scope == RefreshScope::Full;
        self.refreshing = false;
        match data.status {
            Ok(status) => self.apply_status(status, cx),
            Err(e) => self.error = Some(e.to_string()),
        }
        if let Some(Ok(b)) = data.branches {
            self.branches = b;
        }
        self.last_fetch = data.last_fetch;
        if full {
            self.reload_push_state(cx);
            self.reload_stashes(cx);
            if self.pr.open {
                self.load_pr_comparison(cx);
            }
        }
        self.apply_progress(data.paused, cx);
        if full && (!self.commits.is_empty() || self.tab == Tab::History) {
            self.reload_history(cx);
        }
        cx.notify();
        if let Some(next) = self.pending_refresh.take() {
            self.request_refresh(next, cx);
        }
    }

    /// Store the paused operation; announce git's prepared message when a
    /// different operation newly paused (the workspace fills the commit box).
    fn apply_progress(&mut self, paused: Option<PausedOp>, cx: &mut Context<Self>) {
        let (new, msg) = match paused {
            Some(p) => (Some(p.op), p.message),
            None => (None, None),
        };
        let newly = new.is_some()
            && self.merge.in_progress.as_ref().map(std::mem::discriminant) != new.as_ref().map(std::mem::discriminant);
        self.merge.in_progress = new;
        if let (true, Some((summary, description))) = (newly, msg) {
            cx.emit(crate::features::merge::message_event(summary, description));
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
        self.partial.retain(|p, _| paths.contains(p.as_str()));
        let changed = self.status.as_ref().map(|s| &s.files) != Some(&status.files);
        self.status = Some(status);
        self.selected_change = selected_path
            .and_then(|p| self.changes().iter().position(|c| c.path == p))
            .or(if self.changes().is_empty() { None } else { Some(0) });
        if changed || self.change_diff.is_none() {
            self.load_change_diff(cx);
        }
    }
}
