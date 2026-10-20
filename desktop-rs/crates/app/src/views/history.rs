//! History tab: paged commit list (sidebar) + commit details/files/diff (main).

use gd_ui::{Icon, Theme, icon, pressable, vdivider};
use gpui::{
    Context, Entity, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px,
    uniform_list,
};

use super::{relative_time, split_path, status_color};
use crate::actions::LIST;
use crate::repo::RepoModel;
use crate::workspace::Workspace;

const COMMIT_ROW_HEIGHT: f32 = 50.0;

impl Workspace {
    pub fn render_history_list(
        &mut self,
        repo: &Entity<RepoModel>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = Theme::of(cx).clone();
        let n = repo.read(cx).commits.len();
        if n == 0 {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.0))
                .text_color(theme.text_faint)
                .child("No commits yet")
                .into_any_element();
        }
        let list_repo = repo.clone();
        div()
            .key_context(LIST)
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .py(px(4.0))
            .child(
                uniform_list("history", n, move |range, _w, cx| {
                    // Infinite scroll: page in more commits near the end.
                    if range.end + 30 >= n {
                        list_repo.update(cx, |r, cx| r.load_more_history(cx));
                    }
                    let theme = Theme::of(cx).clone();
                    let r = list_repo.read(cx);
                    range
                        .filter_map(|ix| {
                            let c = r.commits.get(ix)?;
                            let selected = r.selected_commit == Some(ix);
                            let repo = list_repo.clone();
                            Some(
                                pressable(("commit", ix), selected, cx)
                                    .flex()
                                    .flex_col()
                                    .justify_center()
                                    .gap(px(3.0))
                                    .h(px(COMMIT_ROW_HEIGHT))
                                    .w_full()
                                    .px(px(10.0))
                                    .rounded(px(6.0))
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(13.0))
                                            .text_color(if selected { theme.text } else { theme.text_muted })
                                            .child(c.summary.clone()),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(11.5))
                                            .text_color(theme.text_faint)
                                            .child(format!("{} • {}", c.author_name, relative_time(c.time))),
                                    )
                                    .on_click(move |_, _, cx| {
                                        repo.update(cx, |r, cx| r.select_commit(ix, cx))
                                    }),
                            )
                        })
                        .collect()
                })
                .track_scroll(&self.history_scroll)
            .px(px(6.0))
                .flex_1(),
            )
            .into_any_element()
    }

    pub fn render_history_main(
        &mut self,
        repo: &Entity<RepoModel>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = Theme::of(cx).clone();
        let r = repo.read(cx);
        let Some(commit) = r.selected_commit.and_then(|ix| r.commits.get(ix)).cloned() else {
            return div().flex_1().into_any_element();
        };
        let n_files = r.commit_files.len();
        let diff = r.commit_diff.clone();
        let file_key = r
            .selected_commit_file
            .and_then(|ix| r.commit_files.get(ix))
            .map(|f| f.path.clone())
            .unwrap_or_default();
        let list_repo = repo.clone();

        let header = div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(6.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .text_size(px(15.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(commit.summary.clone()),
            )
            .when(!commit.body.is_empty(), |d| {
                d.child(
                    div()
                        .max_h(px(120.0))
                        .overflow_hidden()
                        .text_size(px(12.5))
                        .text_color(theme.text_muted)
                        .child(commit.body.clone()),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.0))
                    .text_size(px(12.0))
                    .text_color(theme.text_faint)
                    .child(commit.author_name.clone())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(icon(Icon::Tag).size(px(12.0)).text_color(theme.text_faint))
                            .font_family(theme.font_mono.clone())
                            .child(commit.short_sha.clone()),
                    )
                    .child(relative_time(commit.time))
                    .child(format!("{n_files} changed file{}", if n_files == 1 { "" } else { "s" })),
            );

        let files = uniform_list("commit-files", n_files, move |range, _w, cx| {
            let theme = Theme::of(cx).clone();
            let r = list_repo.read(cx);
            range
                .filter_map(|ix| {
                    let f = r.commit_files.get(ix)?;
                    let selected = r.selected_commit_file == Some(ix);
                    let (dir, name) = split_path(&f.path);
                    let repo = list_repo.clone();
                    Some(
                        pressable(("commit-file", ix), selected, cx)
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
                                    .child(div().flex_none().text_color(if selected { theme.text } else { theme.text_muted }).child(name.to_string())),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .w(px(14.0))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(status_color(f.status, cx))
                                    .child(f.status.badge()),
                            )
                            .on_click(move |_, _, cx| repo.update(cx, |r, cx| r.select_commit_file(ix, cx))),
                    )
                })
                .collect()
        })
        .track_scroll(&self.commit_files_scroll)
            .px(px(6.0))
        .flex_1();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .child(header)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .key_context(LIST)
                            .flex()
                            .flex_col()
                            .flex_none()
                            .w(px(260.0))
                            .py(px(4.0))
                            .child(files),
                    )
                    .child(vdivider(cx))
                    .child(self.render_diff(
                        diff,
                        SharedString::from(format!("commit-diff-{}-{file_key}", commit.sha)),
                        cx,
                    )),
            )
            .into_any_element()
    }
}
