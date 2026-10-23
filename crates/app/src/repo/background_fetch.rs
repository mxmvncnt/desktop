//! Periodic background `git fetch` (Desktop does this hourly).

use std::time::{Duration, Instant};

use gpui::{AppContext, Context};

use super::state::{RemoteOp, RepoModel};

/// Background fetch cadence (Desktop: every 1h; the task asks for 30 min).
const BACKGROUND_FETCH_INTERVAL: Duration = Duration::from_secs(30 * 60);
const BACKGROUND_FETCH_FIRST_CHECK: Duration = Duration::from_secs(10);
/// How often the background task wakes up (also re-renders "Last fetched").
const BACKGROUND_TICK: Duration = Duration::from_secs(60);

impl RepoModel {
    /// Fetch every 30 minutes, first check shortly after opening. Errors are
    /// only logged. Wakes once a minute (also to refresh "Last fetched Nm ago").
    pub(super) fn start_background_fetch(&mut self, cx: &mut Context<Self>) {
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

    fn fetch_is_due(&self) -> bool {
        let due = |t: Option<Duration>| t.is_none_or(|d| d >= BACKGROUND_FETCH_INTERVAL);
        let since_fetch = self.last_fetch.and_then(|t| t.elapsed().ok());
        let since_attempt = self.last_background_fetch.map(|t| t.elapsed());
        due(since_fetch) && due(since_attempt)
    }

    fn maybe_background_fetch(&mut self, cx: &mut Context<Self>) {
        if self.remote_op.is_some() || !self.fetch_is_due() {
            return;
        }
        self.last_background_fetch = Some(Instant::now());
        let repo = self.repo.clone();
        cx.spawn(async move |this, cx| {
            let has_remote = cx
                .background_spawn(async move { repo.remotes().map(|r| !r.is_empty()).unwrap_or(false) })
                .await;
            if has_remote {
                this.update(cx, |this, cx| this.fetch_in_background(cx)).ok();
            }
        })
        .detach();
    }

    fn fetch_in_background(&mut self, cx: &mut Context<Self>) {
        // Another remote op may have started meanwhile.
        if self.remote_op.is_some() {
            return;
        }
        log::info!("background fetch: {}", self.repo.name());
        self.remote_op = Some(RemoteOp::Fetch);
        cx.notify();
        let repo = self.repo.clone();
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
    }
}
