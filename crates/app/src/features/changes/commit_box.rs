//! Summary + description inputs and the commit button.

use gd_ui::{Button, Icon, Theme};
use gpui::{App, Entity, IntoElement, ParentElement, RenderOnce, StatefulInteractiveElement, Styled, Window, div, px};
use gpui_elements::editable_text::EditableTextState;

use super::amend_notice::AmendNotice;
use super::state::CommitButton;
use crate::shared::text_field::text_field;
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct CommitBox {
    button: CommitButton,
    summary: Entity<EditableTextState>,
    description: Entity<EditableTextState>,
    ws: Entity<Workspace>,
}

impl CommitBox {
    pub fn new(
        button: CommitButton,
        summary: Entity<EditableTextState>,
        description: Entity<EditableTextState>,
        ws: Entity<Workspace>,
    ) -> Self {
        Self { button, summary, description, ws }
    }

    fn description_input(&self, theme: &Theme) -> impl IntoElement {
        gpui_elements::editable_text::text_area("description")
            .state(self.description.downgrade())
            .placeholder("Description")
            .placeholder_color(theme.text_faint)
            .selection_color(theme.selection)
            .caret_color(theme.caret)
            .caret_blink_interval_500ms()
            .w_full()
            .h(px(88.0))
            .px(px(10.0))
            .py(px(8.0))
            .rounded(px(gd_ui::theme::CONTROL_RADIUS + 1.0))
            .bg(theme.input_bg)
            .border_1()
            .border_color(theme.border)
            .text_size(px(13.0))
            .text_color(theme.text)
            .overflow_y_scroll()
    }
}

impl RenderOnce for CommitBox {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let b = &self.button;
        let amend_notice = b.amending.then(|| AmendNotice::new(b.head_pushed, self.ws.clone()));
        let commit = Button::new("commit")
            .primary()
            .full_width()
            .icon(if b.amending { Icon::Pen } else { Icon::Check })
            .label(b.label.clone())
            .disabled(!b.enabled)
            .on_click(|_, w, cx| w.dispatch_action(Box::new(crate::actions::Commit), cx));
        div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(8.0))
            .p(px(12.0))
            .border_t_1()
            .border_color(theme.border)
            .children(amend_notice)
            .child(text_field("summary", &self.summary, self.button.default_summary.as_deref().unwrap_or("Summary (required)"), cx))
            .child(self.description_input(&theme))
            .child(commit)
    }
}
