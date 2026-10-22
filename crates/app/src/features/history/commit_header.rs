//! Header of the commit detail pane: summary, body, author, sha, time, file count.

use gd_git::Commit;
use gd_ui::{Icon, Theme, icon};
use gpui::{AnyElement, App, FontWeight, IntoElement, RenderOnce, Window, div, prelude::*, px};

use crate::shared::relative_time::relative_time;
use crate::workspace::plural;

#[derive(IntoElement)]
pub struct CommitHeader {
    commit: Commit,
    n_files: usize,
    /// Trailing control (the diff options button).
    trailing: AnyElement,
}

impl CommitHeader {
    pub fn new(commit: Commit, n_files: usize, trailing: impl IntoElement) -> Self {
        Self { commit, n_files, trailing: trailing.into_any_element() }
    }
}

impl RenderOnce for CommitHeader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let body = self.commit.body.clone();
        let meta = self.meta_line(&theme);
        div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(6.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(theme.border)
            .child(self.title_line())
            .when(!body.is_empty(), |d| {
                d.child(
                    div()
                        .max_h(px(120.0))
                        .overflow_hidden()
                        .text_size(px(12.5))
                        .text_color(theme.text_muted)
                        .child(body),
                )
            })
            .child(meta)
    }
}

impl CommitHeader {
    fn title_line(self) -> impl IntoElement {
        div()
            .flex()
            .items_start()
            .gap(px(8.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(15.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.commit.summary),
            )
            .child(self.trailing)
    }

    fn meta_line(&self, theme: &Theme) -> impl IntoElement + use<> {
        let c = &self.commit;
        div()
            .flex()
            .items_center()
            .gap(px(14.0))
            .text_size(px(12.0))
            .text_color(theme.text_faint)
            .child(c.author_name.clone())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(icon(Icon::Tag).size(px(12.0)).text_color(theme.text_faint))
                    .font_family(theme.font_mono.clone())
                    .child(c.short_sha.clone()),
            )
            .child(relative_time(c.time))
            .child(format!("{} changed file{}", self.n_files, plural(self.n_files as u32)))
    }
}
