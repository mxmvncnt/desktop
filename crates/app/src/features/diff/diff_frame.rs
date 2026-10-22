//! Shared frame around either diff list: whitespace note on top, truncation
//! warning below, wheel and drag handling in between.

use gd_ui::{Theme, motion};
use gpui::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    div, prelude::*, px,
};

use super::text_flow::TextFlow;
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct DiffFrame {
    pub key: SharedString,
    pub list: AnyElement,
    pub truncated: bool,
    pub flow: TextFlow,
    pub note: Option<AnyElement>,
    pub ws: Entity<Workspace>,
}

impl RenderOnce for DiffFrame {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx);
        let wrap = self.flow == TextFlow::Wrap;
        let (ws, ws_wheel) = (self.ws.clone(), self.ws);
        motion::fade_quick(
            self.key,
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .children(self.note)
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        // A gutter drag ends wherever the button is released.
                        .on_mouse_up_all(move |_, _, _, _, cx| {
                            ws.update(cx, |w, _| w.diff_select.drag = None);
                        })
                        // Shift+wheel arrives as a horizontal delta on every platform.
                        .when(!wrap, |d| {
                            d.on_scroll_wheel(move |e, window, cx| {
                                let dx = e.delta.pixel_delta(window.line_height()).x;
                                if dx != px(0.0) {
                                    ws_wheel.update(cx, |w, cx| w.scroll_diff_x(-dx, window, cx));
                                }
                            })
                        })
                        .child(self.list),
                )
                .when(self.truncated, |d| d.child(truncated_warning(theme))),
        )
    }
}

fn truncated_warning(theme: &Theme) -> impl IntoElement {
    div()
        .flex_none()
        .px(px(16.0))
        .py(px(6.0))
        .border_t_1()
        .border_color(theme.border)
        .text_size(px(12.0))
        .text_color(theme.warning)
        .child("This diff is too large to show completely; it was truncated.")
}
