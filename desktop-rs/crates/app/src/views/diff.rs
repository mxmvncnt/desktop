//! Virtualized unified diff. Each row is one `DiffLine`; only visible rows
//! are built per frame, so a 50k-line diff costs the same as a 50-line one.

use std::sync::Arc;

use gd_git::{Diff, DiffLineKind};
use gd_ui::{Theme, motion, theme::DIFF_ROW_HEIGHT};
use gpui::{
    Context, ElementId, IntoElement, ParentElement, Styled, div, prelude::*, px, uniform_list,
};

use crate::workspace::Workspace;

impl Workspace {
    pub fn render_diff(
        &mut self,
        diff: Option<Arc<Diff>>,
        key: impl Into<ElementId>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = Theme::of(cx).clone();
        let Some(diff) = diff else {
            return div().flex_1().into_any_element();
        };
        let message = if diff.is_binary {
            Some("Binary file not shown")
        } else if diff.lines.is_empty() {
            Some("No content changes (mode change or empty file)")
        } else {
            None
        };
        if let Some(message) = message {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.text_faint)
                .child(message)
                .into_any_element();
        }
        let n = diff.lines.len();
        let truncated = diff.truncated;
        let gutter_digits = diff
            .lines
            .iter()
            .filter_map(|l| l.new_line.max(l.old_line))
            .max()
            .unwrap_or(1)
            .to_string()
            .len()
            .max(3);
        let gutter_w = px(gutter_digits as f32 * 7.5 + 16.0);
        let list = uniform_list("diff-lines", n, move |range, _w, cx| {
            let theme = Theme::of(cx).clone();
            range
                .map(|ix| {
                    let line = &diff.lines[ix];
                    let (bg, fg, marker) = match line.kind {
                        DiffLineKind::Add => (theme.diff_add_bg, theme.text, "+"),
                        DiffLineKind::Delete => (theme.diff_del_bg, theme.text, "-"),
                        DiffLineKind::Hunk => (theme.diff_hunk_bg, theme.text_faint, ""),
                        DiffLineKind::NoNewline => (gpui::transparent_black(), theme.text_faint, ""),
                        DiffLineKind::Context => (gpui::transparent_black(), theme.text_muted, " "),
                    };
                    let marker_color = match line.kind {
                        DiffLineKind::Add => theme.diff_add,
                        DiffLineKind::Delete => theme.diff_del,
                        _ => theme.text_faint,
                    };
                    let num = |n: Option<u32>| n.map(|n| n.to_string()).unwrap_or_default();
                    div()
                        .flex()
                        .h(px(DIFF_ROW_HEIGHT))
                        .w_full()
                        .bg(bg)
                        .whitespace_nowrap()
                        .child(
                            div()
                                .flex_none()
                                .w(gutter_w)
                                .pr(px(8.0))
                                .flex()
                                .justify_end()
                                .text_color(theme.diff_gutter)
                                .child(num(line.old_line)),
                        )
                        .child(
                            div()
                                .flex_none()
                                .w(gutter_w)
                                .pr(px(8.0))
                                .flex()
                                .justify_end()
                                .text_color(theme.diff_gutter)
                                .border_r_1()
                                .border_color(theme.border)
                                .child(num(line.new_line)),
                        )
                        .child(
                            div()
                                .flex_none()
                                .w(px(20.0))
                                .flex()
                                .justify_center()
                                .text_color(marker_color)
                                .child(marker),
                        )
                        .child(div().text_color(fg).child(line.text.clone()))
                })
                .collect()
        })
        .with_horizontal_sizing_behavior(gpui::ListHorizontalSizingBehavior::Unconstrained)
        .track_scroll(&self.diff_scroll)
        .size_full()
        .font_family(theme.font_mono.clone())
        .text_size(px(12.0))
        .line_height(px(DIFF_ROW_HEIGHT));
        motion::fade_quick(
            key,
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .child(list)
                .when(truncated, |d| {
                    d.child(
                        div()
                            .flex_none()
                            .px(px(16.0))
                            .py(px(6.0))
                            .border_t_1()
                            .border_color(theme.border)
                            .text_size(px(12.0))
                            .text_color(theme.warning)
                            .child("This diff is too large to show completely; it was truncated."),
                    )
                }),
        )
        .into_any_element()
    }
}

