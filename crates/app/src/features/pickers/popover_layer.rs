//! Backdrop + positioned card hosting whichever popover is open.

use gd_ui::{motion, theme::{TOOLBAR_HEIGHT, scrim}};
use gpui::{
    AnyElement, App, Context, Entity, InteractiveElement, IntoElement, MouseButton,
    ParentElement, RenderOnce, Styled, Window, deferred, div, px,
};

use crate::workspace::{Popover, Workspace};

#[derive(IntoElement)]
pub struct PopoverLayer {
    ws: Entity<Workspace>,
    popover: Popover,
    generation: usize,
    content: AnyElement,
}

impl PopoverLayer {
    /// `None` when no popover is open.
    pub fn open(ws: &mut Workspace, cx: &mut Context<Workspace>) -> Option<Self> {
        let popover = ws.popover?;
        let content = match popover {
            Popover::Repositories => ws.repo_picker(cx).into_any_element(),
            Popover::Branches => match ws.branch_picker(cx) {
                Some(picker) => picker.into_any_element(),
                None => div().into_any_element(),
            },
            Popover::NewBranch => ws.new_branch_form(cx).into_any_element(),
            Popover::Clone => ws.clone_form(cx).into_any_element(),
        };
        Some(Self { ws: cx.entity(), popover, generation: ws.popover_gen, content })
    }
}

impl RenderOnce for PopoverLayer {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (left, width) = match self.popover {
            Popover::Repositories | Popover::Clone => (8.0, 380.0),
            Popover::Branches | Popover::NewBranch => (296.0, 380.0),
        };
        let ws = self.ws;
        deferred(
            div()
                .absolute()
                .inset_0()
                .child(
                    // Backdrop: click outside closes. Very light scrim like zeron menus.
                    div()
                        .id("popover-backdrop")
                        .absolute()
                        .inset_0()
                        .bg(scrim(0.18))
                        .on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                            ws.update(cx, |this, cx| {
                                this.popover_dismissed = this.popover.map(|p| (p, ev.position));
                                this.close_popover(window, cx)
                            })
                        }),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(TOOLBAR_HEIGHT - 2.0))
                        .left(px(left))
                        .w(px(width))
                        .child(motion::menu_in(
                            ("popover", self.generation),
                            gd_ui::overlay_card(cx)
                                .id("popover-card")
                                .occlude()
                                .child(self.content),
                        )),
                ),
        )
    }
}
