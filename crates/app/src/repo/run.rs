//! The generic write-op helper every feature's `ops.rs` builds on.

use gd_git::Repository;
use gpui::{AppContext, Context};

use super::state::RepoModel;

impl RepoModel {
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
}
