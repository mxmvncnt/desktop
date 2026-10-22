//! Background syntax highlighting of the displayed diffs.

use std::sync::Arc;

use gd_git::Diff;
use gpui::{AppContext, Context, Task};

use super::state::RepoModel;
use crate::features::diff::{ExpandTarget, highlight_diff};

impl RepoModel {
    /// Highlight `diff` on the background executor. The result is applied only
    /// if `diff` is still the displayed one (`Arc::ptr_eq`); a newer diff
    /// replaces the stored task, which cancels this one if it hasn't started.
    pub(super) fn spawn_highlight(
        &self,
        diff: Arc<Diff>,
        path: String,
        side: ExpandTarget,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        let cache = self.hl_cache.clone();
        let for_task = diff.clone();
        cx.spawn(async move |this, cx| {
            let hl = cx
                .background_spawn(async move { highlight_diff(&for_task, &path, &cache) })
                .await;
            this.update(cx, |this, cx| {
                let (slot, current) = match side {
                    ExpandTarget::Commit => (&mut this.commit_hl, &this.commit_diff),
                    ExpandTarget::Changes => (&mut this.change_hl, &this.change_diff),
                };
                if hl.is_some() && current.as_ref().is_some_and(|c| Arc::ptr_eq(c, &diff)) {
                    *slot = hl;
                    cx.notify();
                }
            })
            .ok();
        })
    }
}
