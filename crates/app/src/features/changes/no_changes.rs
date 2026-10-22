//! "No local changes" pane with push / pull / open-folder suggestions.

use gd_ui::{Icon, Theme, motion};
use gpui::{App, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};

use crate::repo::RepoModel;
use crate::shared::suggestion_card::SuggestionCard;
use crate::shared::plural::plural;

#[derive(IntoElement)]
pub struct NoChanges {
    name: String,
    root: String,
    /// Commits ahead / behind; only known when the branch has an upstream.
    ahead_behind: Option<(u32, u32)>,
}

impl NoChanges {
    pub fn new(r: &RepoModel) -> Self {
        let ahead_behind = r
            .status
            .as_ref()
            .filter(|s| s.upstream.is_some())
            .map(|s| (s.ahead_behind.ahead, s.ahead_behind.behind));
        Self { name: r.repo.name(), root: r.repo.root().display().to_string(), ahead_behind }
    }

    fn suggestions(&self) -> impl IntoElement {
        let (ahead, behind) = self.ahead_behind.unwrap_or_default();
        let root = self.root.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .w(px(460.0))
            .children((ahead > 0).then(|| {
                SuggestionCard::new(
                    "push-suggestion",
                    Icon::ArrowUp,
                    "Push commits to the remote",
                    format!("You have {ahead} local commit{} waiting to be pushed.", plural(ahead)),
                    "Push",
                    |_, w, cx| w.dispatch_action(Box::new(crate::actions::Push), cx),
                )
            }))
            .children((behind > 0).then(|| {
                SuggestionCard::new(
                    "pull-suggestion",
                    Icon::ArrowDown,
                    "Pull commits from the remote",
                    format!("The remote has {behind} commit{} you don't have.", plural(behind)),
                    "Pull",
                    |_, w, cx| w.dispatch_action(Box::new(crate::actions::Pull), cx),
                )
            }))
            .child(SuggestionCard::new(
                "open-folder",
                Icon::Folder,
                "View the files of your repository",
                self.root.clone(),
                "Open folder",
                move |_, _, cx| cx.open_url(&format!("file://{root}")),
            ))
    }
}

impl RenderOnce for NoChanges {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let heading = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.0))
            .child(div().text_size(px(20.0)).font_weight(gpui::FontWeight::SEMIBOLD).child("No local changes"))
            .child(div().text_color(theme.text_muted).child(format!("There are no uncommitted changes in {}.", self.name)));
        div().flex_1().flex().items_center().justify_center().child(motion::fade_in(
            ("no-changes", self.name.len()),
            div().flex().flex_col().items_center().gap(px(20.0)).child(heading).child(self.suggestions()),
        ))
    }
}
