//! Frame shared by the repository and branch pickers: title, filter input,
//! list (or "No matches"), footer. Owns the keyboard routing: the focused
//! filter input turns up/down into NavUp/NavDown, so those are captured here
//! too (the LIST-context bindings never reach a focused input).

use gd_ui::{Theme, divider};
use gpui::{
    AnyElement, App, Context, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce, Styled, Window,
    div, prelude::*, px,
};
use gpui_elements::editable_text::{
    EditableTextState,
    actions::{Enter, NavDown, NavUp},
};

use crate::actions::{SelectNext, SelectPrevious};
use crate::workspace::{Workspace, text_field};

pub(super) const PICKER_ROW: f32 = 34.0;
const PICKER_MAX_ROWS: usize = 10;

type EnterHandler = Box<dyn Fn(&mut Workspace, &mut Window, &mut Context<Workspace>)>;

#[derive(IntoElement)]
pub(super) struct PickerShell {
    title: &'static str,
    placeholder: &'static str,
    filter: Entity<EditableTextState>,
    rows: usize,
    list: AnyElement,
    footer: AnyElement,
    on_enter: EnterHandler,
    ws: Entity<Workspace>,
}

impl PickerShell {
    pub fn new(
        title: &'static str,
        placeholder: &'static str,
        filter: Entity<EditableTextState>,
        rows: usize,
        list: impl IntoElement,
        footer: impl IntoElement,
        ws: Entity<Workspace>,
    ) -> Self {
        Self {
            title,
            placeholder,
            filter,
            rows,
            list: list.into_any_element(),
            footer: footer.into_any_element(),
            on_enter: Box::new(|_, _, _| {}),
            ws,
        }
    }

    pub fn on_enter(mut self, f: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static) -> Self {
        self.on_enter = Box::new(f);
        self
    }

    fn body(&self, theme: &Theme) -> gpui::Div {
        let height = (self.rows.clamp(1, PICKER_MAX_ROWS) as f32) * PICKER_ROW + 8.0;
        div().h(px(height)).py(px(4.0)).when(self.rows == 0, |d| {
            d.flex()
                .items_center()
                .justify_center()
                .text_size(px(12.0))
                .text_color(theme.text_faint)
                .child("No matches")
        })
    }
}

impl RenderOnce for PickerShell {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let body = self.body(&theme).when(self.rows > 0, |d| d.child(self.list));
        let ws = self.ws;
        let on_enter = self.on_enter;
        let enter_ws = ws.clone();
        let step = move |delta: isize| {
            let ws = ws.clone();
            move |cx: &mut App| {
                cx.stop_propagation();
                ws.update(cx, |this, cx| this.step_picker(delta, cx))
            }
        };
        let (down, up, next, prev) = (step(1), step(-1), step(1), step(-1));
        div()
            .flex()
            .flex_col()
            .capture_action(move |_: &Enter, w, cx| {
                cx.stop_propagation();
                enter_ws.update(cx, |this, cx| on_enter(this, w, cx))
            })
            .capture_action(move |_: &NavDown, _, cx| down(cx))
            .capture_action(move |_: &NavUp, _, cx| up(cx))
            .capture_action(move |_: &SelectNext, _, cx| next(cx))
            .capture_action(move |_: &SelectPrevious, _, cx| prev(cx))
            .child(
                div()
                    .px(px(14.0))
                    .pt(px(12.0))
                    .pb(px(8.0))
                    .text_size(px(12.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text_muted)
                    .child(self.title),
            )
            .child(
                div()
                    .px(px(10.0))
                    .pb(px(10.0))
                    .child(text_field("picker-filter", &self.filter, self.placeholder, cx)),
            )
            .child(divider(cx))
            .child(body)
            .child(divider(cx))
            .child(div().flex().gap(px(6.0)).p(px(8.0)).bg(gd_ui::theme::ink(0.02)).child(self.footer))
    }
}
