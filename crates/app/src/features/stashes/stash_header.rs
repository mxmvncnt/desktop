//! Header of the stash detail pane: title, metadata and Apply / Restore / Discard.

use gd_git::Stash;
use gd_ui::{Button, Icon, Theme, badge, icon};
use gpui::{App, Entity, IntoElement, RenderOnce, Window, div, prelude::*, px};

use super::state::StashDialog;
use crate::repo::RepoModel;
use crate::shared::relative_time::relative_time;
use crate::shared::plural::plural;
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct StashHeader {
    stash: Stash,
    n_files: usize,
    /// Made on the checked-out branch.
    here: bool,
    repo: Entity<RepoModel>,
    ws: Entity<Workspace>,
}

impl StashHeader {
    pub fn new(stash: Stash, n_files: usize, here: bool, repo: Entity<RepoModel>, ws: Entity<Workspace>) -> Self {
        Self { stash, n_files, here, repo, ws }
    }
}

impl RenderOnce for StashHeader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(16.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(theme.border)
            .child(self.summary(&theme, cx))
            .child(self.buttons())
    }
}

impl StashHeader {
    fn summary(&self, theme: &Theme, cx: &App) -> impl IntoElement + use<> {
        let s = &self.stash;
        let (here, accent) = (self.here, if self.here { theme.accent } else { theme.text_faint });
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(6.0))
            .child(div().truncate().text_size(px(15.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(s.title()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .text_size(px(12.0))
                    .text_color(theme.text_faint)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(5.0))
                            .child(icon(Icon::GitBranch).size(px(12.0)).text_color(accent))
                            .child(s.branch.clone().unwrap_or_else(|| "(no branch)".into())),
                    )
                    .child(
                        badge(if here { "this branch" } else { "other branch" }, cx)
                            .text_color(if here { theme.accent } else { theme.text_muted }),
                    )
                    .child(format!("{} • {}", s.date, relative_time(s.time)))
                    .child(format!("{} file{}", self.n_files, plural(self.n_files as u32)))
                    .child(div().font_family(theme.font_mono.clone()).child(s.short_sha.clone())),
            )
    }

    fn buttons(&self) -> impl IntoElement + use<> {
        let (repo_a, repo_b, ws) = (self.repo.clone(), self.repo.clone(), self.ws.clone());
        let (sha_a, sha_b, stash) = (self.stash.sha.clone(), self.stash.sha.clone(), self.stash.clone());
        div()
            .flex()
            .flex_none()
            .gap(px(8.0))
            .child(
                Button::new("stash-apply")
                    .icon(Icon::ArchiveUp)
                    .label("Apply")
                    .on_click(move |_, _, cx| repo_a.update(cx, |r, cx| r.stash_apply(sha_a.clone(), cx))),
            )
            .child(
                Button::new("stash-pop")
                    .primary()
                    .icon(Icon::Restart)
                    .label("Restore")
                    .on_click(move |_, _, cx| repo_b.update(cx, |r, cx| r.stash_pop(sha_b.clone(), cx))),
            )
            .child(
                Button::new("stash-drop")
                    .ghost()
                    .icon(Icon::Trash)
                    .label("Discard")
                    .on_click(move |_, w, cx| {
                        let s = stash.clone();
                        ws.update(cx, |this, cx| this.open_stash_dialog(StashDialog::Drop(s), w, cx))
                    }),
            )
    }
}
