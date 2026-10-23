//! Thin drag handle on the sidebar's right edge. Dragging it resizes the
//! sidebar (see `resize.rs`); double-click resets the width.

use gd_ui::Theme;
use gpui::{
    App, AppContext as _, ClickEvent, ColorExt as _, CursorStyle, Entity, InteractiveElement, IntoElement, MouseButton,
    Render, RenderOnce, StatefulInteractiveElement, Styled, Window, div, px,
};

use crate::workspace::Workspace;

/// Drag payload marking an in-progress sidebar resize.
#[derive(Clone)]
pub struct SidebarResize;

/// The (invisible) view GPUI shows under the cursor while dragging.
struct NoGhost;

impl Render for NoGhost {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}

#[derive(IntoElement)]
pub struct SidebarHandle {
    ws: Entity<Workspace>,
}

impl SidebarHandle {
    pub fn new(ws: Entity<Workspace>) -> Self {
        Self { ws }
    }
}

impl RenderOnce for SidebarHandle {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let accent = Theme::of(cx).accent;
        let ws = self.ws;
        div()
            .id("sidebar-handle")
            .absolute()
            .top_0()
            .bottom_0()
            .right(px(-3.0))
            .w(px(6.0))
            .cursor(CursorStyle::ResizeLeftRight)
            .hover_style(move |s| s.bg(accent.opacity(0.5)))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_drag(SidebarResize, |_, _, _, cx| cx.new(|_| NoGhost))
            .on_click(move |e: &ClickEvent, _, cx| {
                if e.click_count() == 2 {
                    ws.update(cx, |this, cx| this.reset_sidebar_width(cx));
                }
            })
    }
}
