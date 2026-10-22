//! Desktop's "Your changes will modify your most recent commit" notice, with
//! a "Stop amending" link and a force-push warning for pushed commits.

use gd_ui::{Icon, Theme, icon, motion};
use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, StatefulInteractiveElement, Styled, Window, div,
    prelude::*, px,
};

use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct AmendNotice {
    pushed: bool,
    ws: gpui::Entity<Workspace>,
}

impl AmendNotice {
    pub fn new(pushed: bool, ws: gpui::Entity<Workspace>) -> Self {
        Self { pushed, ws }
    }
}

impl RenderOnce for AmendNotice {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let pushed = self.pushed;
        let ws = self.ws;
        let glyph = if pushed { Icon::Warning } else { Icon::Info };
        let glyph_color = if pushed { theme.warning } else { theme.text_muted };
        motion::fade_quick(
            "amend-notice",
            div()
                .flex()
                .items_start()
                .gap(px(8.0))
                .p(px(10.0))
                .rounded(px(gd_ui::theme::CONTROL_RADIUS + 1.0))
                .bg(gd_ui::theme::ink(0.04))
                .border_1()
                .border_color(theme.border)
                .child(icon(glyph).size(px(14.0)).text_color(glyph_color))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .text_size(px(12.0))
                        .text_color(theme.text_muted)
                        .child("Your changes will modify your most recent commit.")
                        .when(pushed, |d| {
                            d.child(
                                div()
                                    .text_color(theme.warning)
                                    .child("It was already pushed: you'll need to force push afterwards."),
                            )
                        })
                        .child(
                            div()
                                .id("stop-amending")
                                .cursor_pointer()
                                .text_color(theme.text)
                                .underline()
                                .child("Stop amending")
                                .on_click(move |_, _, cx| ws.update(cx, |this, cx| this.stop_amend(cx))),
                        ),
                ),
        )
    }
}
