//! Header of the commit detail pane: summary, body, author, sha, time, file count.

use gd_git::Commit;
use gd_ui::{Icon, Theme, icon};
use gpui::{AnyElement, App, DragMoveEvent, Entity, FontWeight, IntoElement, RenderOnce, Window, div, prelude::*, px};

use super::message_handle::{Drag, MessageHandle};
use super::message_resize::MSG_DEFAULT_MAX;
use crate::workspace::Workspace;
use crate::shared::commit_authors::{authors_text, body_without_trailers};
use crate::shared::relative_time::relative_time;
use crate::shared::plural::plural;

#[derive(IntoElement)]
pub struct CommitHeader {
    commit: Commit,
    n_files: usize,
    /// Trailing control (the diff options button).
    trailing: AnyElement,
    /// User-set height of the message area (`None` = fit up to the default).
    msg_height: Option<f32>,
    ws: Entity<Workspace>,
}

impl CommitHeader {
    pub fn new(
        commit: Commit,
        n_files: usize,
        trailing: impl IntoElement,
        msg_height: Option<f32>,
        ws: Entity<Workspace>,
    ) -> Self {
        Self { commit, n_files, trailing: trailing.into_any_element(), msg_height, ws }
    }
}

impl RenderOnce for CommitHeader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let body = body_without_trailers(&self.commit.body);
        let meta = self.meta_line(&theme);
        let message = (!body.is_empty()).then(|| self.message_area(body, &theme));
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
            .children(message)
            .child(meta)
    }
}

impl CommitHeader {
    /// Scrollable message with a resize handle below it.
    fn message_area(&self, body: String, theme: &Theme) -> impl IntoElement + use<> {
        let text = div()
            .id("commit-message")
            .overflow_y_scroll()
            .text_size(px(12.5))
            .text_color(theme.text_muted)
            .when_some(self.msg_height, |d, h| d.h(px(h)))
            .when(self.msg_height.is_none(), |d| d.max_h(px(MSG_DEFAULT_MAX)))
            .child(body);
        let ws = self.ws.clone();
        let shown = self.msg_height.unwrap_or(MSG_DEFAULT_MAX);
        div()
            .flex()
            .flex_col()
            .on_drag_move(move |e: &DragMoveEvent<Drag>, _, cx| {
                let y = e.event.position.y.as_f32();
                ws.update(cx, |this, cx| this.drag_commit_message_to(y, shown, cx))
            })
            .child(text)
            .child(MessageHandle::new(self.ws.clone()))
    }

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
            .child(div().min_w_0().truncate().child(authors_text(c)))
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
