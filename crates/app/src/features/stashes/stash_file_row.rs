//! One file of the selected stash: directory, name and status badge.

use gd_git::FileChange;
use gd_ui::{Theme, pressable};
use gpui::{App, Entity, IntoElement, RenderOnce, Window, div, prelude::*, px};

use crate::repo::RepoModel;
use crate::views::{split_path, status_color};

#[derive(IntoElement)]
pub struct StashFileRow {
    ix: usize,
    change: FileChange,
    selected: bool,
    repo: Entity<RepoModel>,
}

impl StashFileRow {
    pub fn new(ix: usize, change: FileChange, selected: bool, repo: Entity<RepoModel>) -> Self {
        Self { ix, change, selected, repo }
    }
}

impl RenderOnce for StashFileRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let (ix, repo) = (self.ix, self.repo);
        let f = &self.change;
        let (dir, name) = split_path(&f.path);
        pressable(("stash-file", ix), self.selected, cx)
            .flex()
            .items_center()
            .gap(px(8.0))
            .h(px(28.0))
            .w_full()
            .px(px(8.0))
            .rounded(px(6.0))
            .text_size(px(12.5))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(div().flex_shrink(1.0).min_w_0().truncate().text_color(theme.text_faint).child(dir.to_string()))
                    .child(
                        div()
                            .flex_none()
                            .text_color(if self.selected { theme.text } else { theme.text_muted })
                            .child(name.to_string()),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(14.0))
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(status_color(f.status, cx))
                    .child(f.status.badge()),
            )
            .on_click(move |_, _, cx| repo.update(cx, |r, cx| r.select_stash_file(ix, cx)))
    }
}
