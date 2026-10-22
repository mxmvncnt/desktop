//! `impl Workspace`: raising and clearing banner messages.

use std::time::Duration;

use gpui::{Context, Entity};

use crate::repo::RepoModel;
use crate::workspace::Workspace;

/// Success notices disappear on their own after this long.
const NOTICE_LIFETIME: Duration = Duration::from_secs(3);

impl Workspace {
    /// Error on the open repo, or the startup error when none is open.
    pub(crate) fn show_error(&mut self, msg: String, cx: &mut Context<Self>) {
        if let Some(repo) = &self.repo {
            repo.update(cx, |r, cx| {
                r.error = Some(msg);
                cx.notify()
            });
        } else {
            log::error!("{msg}");
            self.startup_error = Some(msg);
            cx.notify();
        }
    }

    pub(crate) fn show_notice(&mut self, msg: String, cx: &mut Context<Self>) {
        if let Some(repo) = &self.repo {
            repo.update(cx, |r, cx| {
                r.notice = Some(msg);
                cx.notify()
            });
        }
    }

    pub(crate) fn dismiss_notices(&mut self, cx: &mut Context<Self>) {
        self.startup_error = None;
        if let Some(r) = &self.repo {
            r.update(cx, |r, cx| {
                r.error = None;
                r.notice = None;
                cx.notify()
            });
        }
        cx.notify()
    }
}

/// `cx.observe` callback for the repo model: re-render, and start the
/// auto-dismiss timer when a notice appears.
pub(crate) fn observe_notices(this: &mut Workspace, model: Entity<RepoModel>, cx: &mut Context<Workspace>) {
    if model.read(cx).notice.is_some() && this.notice_task.is_none() {
        let model = model.downgrade();
        this.notice_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(NOTICE_LIFETIME).await;
            model
                .update(cx, |m, cx| {
                    m.notice = None;
                    cx.notify()
                })
                .ok();
            this.update(cx, |this, _| this.notice_task = None).ok();
        }));
    }
    cx.notify()
}
