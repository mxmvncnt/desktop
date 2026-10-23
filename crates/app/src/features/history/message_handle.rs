//! Drag handle on the bottom edge of the commit message area; double-click
//! resets the height.

use gd_ui::Theme;
use gpui::{
    App, ParentElement, AppContext as _, ClickEvent, ColorExt as _, CursorStyle, Entity, InteractiveElement, IntoElement, MouseButton,
    Render, RenderOnce, StatefulInteractiveElement, Styled, Window, div, px,
};

use crate::workspace::Workspace;

/// Drag payload marking an in-progress message-area resize.
#[derive(Clone)]
pub struct MessageDrag;

struct NoGhost;

impl Render for NoGhost {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}

#[derive(IntoElement)]
pub struct MessageHandle {
    ws: Entity<Workspace>,
}

impl MessageHandle {
    pub fn new(ws: Entity<Workspace>) -> Self {
        Self { ws }
    }
}

impl RenderOnce for MessageHandle {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx);
        let (accent, line) = (theme.accent, theme.border);
        let ws = self.ws;
        div()
            .id("commit-message-handle")
            .h(px(8.0))
            .w_full()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .cursor(CursorStyle::ResizeUpDown)
            .group("msg-handle")
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_drag(MessageDrag, |_, _, _, cx| cx.new(|_| NoGhost))
            .on_click(move |e: &ClickEvent, _, cx| {
                if e.click_count() == 2 {
                    ws.update(cx, |this, cx| this.reset_commit_message_height(cx));
                }
            })
            .child(
                div()
                    .w(px(36.0))
                    .h(px(3.0))
                    .rounded(px(2.0))
                    .bg(line)
                    .group_hover_style("msg-handle", move |s| s.bg(accent.opacity(0.6))),
            )
    }
}

pub(super) use MessageDrag as Drag;
