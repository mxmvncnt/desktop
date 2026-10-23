//! The gear button in the diff headers; opens the options popover.

use gd_ui::{Icon, icon_button};
use gpui::{App, Entity, IntoElement, RenderOnce, Window};

use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct DiffOptionsButton {
    ws: Entity<Workspace>,
}

impl DiffOptionsButton {
    pub fn new(ws: Entity<Workspace>) -> Self {
        Self { ws }
    }
}

impl RenderOnce for DiffOptionsButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let ws = self.ws;
        icon_button("diff-options", Icon::Tuning).compact().on_click(move |e, w, cx| {
            let pos = e.position();
            ws.update(cx, |this, cx| this.open_diff_options(pos, w, cx))
        })
    }
}
