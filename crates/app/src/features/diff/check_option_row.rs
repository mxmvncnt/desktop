//! A checkbox row of the options popover and the settings page; clicking the
//! box or the row toggles.

use std::rc::Rc;

use gd_ui::{CheckState, Checkbox, pressable};
use gpui::{App, Context, Entity, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, prelude::*, px};

use crate::workspace::Workspace;

type Toggle = Rc<dyn Fn(&mut Workspace, &mut Context<Workspace>)>;

#[derive(IntoElement)]
pub struct CheckOptionRow {
    id: &'static str,
    text: &'static str,
    checked: bool,
    ws: Entity<Workspace>,
    toggle: Toggle,
}

impl CheckOptionRow {
    pub fn new(
        id: &'static str,
        text: &'static str,
        checked: bool,
        ws: Entity<Workspace>,
        toggle: impl Fn(&mut Workspace, &mut Context<Workspace>) + 'static,
    ) -> Self {
        Self { id, text, checked, ws, toggle: Rc::new(toggle) }
    }
}

impl RenderOnce for CheckOptionRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.id;
        let (ws, ws_check) = (self.ws.clone(), self.ws);
        let (toggle, toggle_check) = (self.toggle.clone(), self.toggle);
        let state = if self.checked { CheckState::Checked } else { CheckState::Unchecked };
        pressable(id, false, cx)
            .flex()
            .items_center()
            .gap(px(10.0))
            .h(px(30.0))
            .px(px(8.0))
            .rounded(px(6.0))
            .text_size(px(13.0))
            .child(
                Checkbox::new(SharedString::from(format!("{id}-check")), state)
                    .on_click(move |_, _, cx| ws_check.update(cx, |this, cx| toggle_check(this, cx))),
            )
            .child(self.text)
            .on_click(move |_, _, cx| ws.update(cx, |this, cx| toggle(this, cx)))
    }
}
