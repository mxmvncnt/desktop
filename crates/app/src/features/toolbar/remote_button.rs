//! What the contextual remote button shows and does for the current repo state.

use gd_git::AheadBehind;
use gd_ui::Icon;
use gpui::SharedString;

use super::last_fetched::last_fetched_caption;
use crate::repo::{RemoteOp, RepoModel};
use crate::shared::plural::plural;

pub(super) struct RemoteButton {
    pub icon: Icon,
    pub caption: SharedString,
    pub title: SharedString,
    /// Op to run on click; `None` while one is in flight or no repo is open.
    pub op: Option<RemoteOp>,
    pub busy: bool,
}

impl RemoteButton {
    pub fn for_repo(repo: Option<&RepoModel>) -> Self {
        match repo {
            None => Self::new(Icon::Refresh, "No repository", "Fetch origin", None),
            Some(r) => Self::for_model(r),
        }
    }

    fn new(icon: Icon, caption: impl Into<SharedString>, title: impl Into<SharedString>, op: Option<RemoteOp>) -> Self {
        Self { icon, caption: caption.into(), title: title.into(), op, busy: false }
    }

    fn busy(mut self) -> Self {
        self.busy = true;
        self
    }

    fn for_model(r: &RepoModel) -> Self {
        let st = r.status.as_ref();
        let upstream = st.and_then(|s| s.upstream.clone());
        let ab = st.map(|s| s.ahead_behind).unwrap_or_default();
        let remote = upstream
            .as_deref()
            .and_then(|u| u.split_once('/'))
            .map(|(r, _)| r.to_string())
            .unwrap_or_else(|| "origin".into());
        if let Some(op) = r.remote_op {
            return Self::in_flight(op, &remote);
        }
        let has_branch = st.and_then(|s| s.branch.as_ref()).is_some();
        Self::idle(r, upstream.is_some(), has_branch, ab, &remote)
    }

    fn in_flight(op: RemoteOp, remote: &str) -> Self {
        match op {
            RemoteOp::Fetch => Self::new(Icon::Refresh, "Fetching…", format!("Fetch {remote}"), None),
            RemoteOp::Pull => Self::new(Icon::ArrowDown, "Pulling…", format!("Pull {remote}"), None),
            RemoteOp::Push => Self::new(Icon::ArrowUp, "Pushing…", format!("Push {remote}"), None),
        }
        .busy()
    }

    fn idle(r: &RepoModel, has_upstream: bool, has_branch: bool, ab: AheadBehind, remote: &str) -> Self {
        if !has_upstream && has_branch {
            return Self::new(
                Icon::Cloud,
                "Publish this branch to the remote",
                "Publish branch",
                Some(RemoteOp::Push),
            );
        }
        if has_upstream && ab.behind > 0 {
            let caption = format!("{} commit{} behind", ab.behind, plural(ab.behind));
            return Self::new(Icon::ArrowDown, caption, format!("Pull {remote}"), Some(RemoteOp::Pull));
        }
        if has_upstream && ab.ahead > 0 {
            let caption = format!("{} commit{} ahead", ab.ahead, plural(ab.ahead));
            return Self::new(Icon::ArrowUp, caption, format!("Push {remote}"), Some(RemoteOp::Push));
        }
        Self::new(
            Icon::Refresh,
            last_fetched_caption(r.last_fetch),
            format!("Fetch {remote}"),
            Some(RemoteOp::Fetch),
        )
    }
}
