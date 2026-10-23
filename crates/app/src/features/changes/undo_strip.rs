//! Desktop's "Committed just now" strip with an Undo button.

use gd_ui::{Button, Icon, Theme, icon, motion};
use gpui::{App, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};

use crate::repo::LastCommit;
use crate::shared::relative_time::relative_time;
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct UndoStrip {
    commit: LastCommit,
    ws: Entity<Workspace>,
}

impl UndoStrip {
    pub fn new(commit: LastCommit, ws: Entity<Workspace>) -> Self {
        Self { commit, ws }
    }
}

impl RenderOnce for UndoStrip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let c = self.commit;
        let ws = self.ws;
        motion::fade_quick(
            ("undo-strip", c.time as usize),
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(10.0))
                .px(px(12.0))
                .py(px(8.0))
                .border_t_1()
                .border_color(theme.border)
                .bg(gd_ui::theme::ink(0.03))
                .child(icon(Icon::Check).size(px(14.0)).text_color(theme.success))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(theme.text_faint)
                                .child(format!("Committed {}", relative_time(c.time))),
                        )
                        .child(div().text_size(px(12.5)).truncate().child(c.summary)),
                )
                .child(
                    Button::new("undo-commit")
                        .compact()
                        .icon(Icon::Restart)
                        .label("Undo")
                        .on_click(move |_, _, cx| ws.update(cx, |this, cx| this.undo_commit(cx))),
                ),
        )
    }
}
