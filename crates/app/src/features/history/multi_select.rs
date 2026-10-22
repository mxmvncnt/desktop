//! Main pane while 2+ commits are selected: count, hints and the commit list.

use gd_git::Commit;
use gd_ui::Theme;
use gpui::{App, Entity, FontWeight, IntoElement, RenderOnce, Window, div, prelude::*, px, uniform_list};

use crate::repo::RepoModel;

#[derive(IntoElement)]
pub struct MultiSelectPane {
    repo: Entity<RepoModel>,
}

impl MultiSelectPane {
    pub fn new(repo: Entity<RepoModel>) -> Self {
        Self { repo }
    }
}

impl RenderOnce for MultiSelectPane {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let r = self.repo.read(cx);
        let commits: Vec<Commit> = r.multi_indices().iter().filter_map(|&i| r.commits.get(i).cloned()).collect();
        let pushed = commits.iter().filter(|c| !r.is_unpushed(&c.sha)).count();
        let n = commits.len();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .child(header(n, pushed, cx))
            .child(
                uniform_list("multi-commits", n, move |range, _w, cx| {
                    range.filter_map(|ix| Some(multi_commit_row(ix, commits.get(ix)?, cx))).collect()
                })
                .flex_1(),
            )
    }
}

fn header(n: usize, pushed: usize, cx: &App) -> impl IntoElement + use<> {
    let theme = Theme::of(cx).clone();
    div()
        .flex()
        .flex_col()
        .flex_none()
        .gap(px(4.0))
        .px(px(16.0))
        .py(px(12.0))
        .border_b_1()
        .border_color(theme.border)
        .child(div().text_size(px(15.0)).font_weight(FontWeight::SEMIBOLD).child(format!("{n} commits selected")))
        .child(div().text_size(px(12.0)).text_color(theme.text_faint).child(
            "Right-click to squash or cherry-pick. Ctrl/Cmd-click toggles a commit, Shift-click selects a range.",
        ))
        .when(pushed > 0, |d| {
            d.child(div().text_size(px(12.0)).text_color(theme.warning).child(format!(
                "{pushed} of them {} already on the remote.",
                if pushed == 1 { "is" } else { "are" }
            )))
        })
}

fn multi_commit_row(ix: usize, c: &Commit, cx: &App) -> impl IntoElement + use<> {
    let theme = Theme::of(cx).clone();
    div()
        .id(("multi-commit", ix))
        .flex()
        .items_center()
        .gap(px(12.0))
        .h(px(32.0))
        .px(px(16.0))
        .text_size(px(13.0))
        .child(
            div()
                .flex_none()
                .w(px(64.0))
                .font_family(theme.font_mono.clone())
                .text_size(px(12.0))
                .text_color(theme.text_faint)
                .child(c.short_sha.clone()),
        )
        .child(div().flex_1().min_w_0().truncate().text_color(theme.text_muted).child(c.summary.clone()))
        .child(div().flex_none().text_size(px(12.0)).text_color(theme.text_faint).child(c.author_name.clone()))
}
