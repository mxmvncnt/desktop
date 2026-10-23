use gd_ui::{Tabs, Theme};
use gpui::{App, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};

use crate::features::diff::CheckOptionRow;
use crate::workspace::Workspace;

/// The diff preferences the page shows.
#[derive(Clone, Copy)]
pub struct DiffOptionValues {
    pub split: bool,
    pub wrap: bool,
    pub hide_whitespace: bool,
}

#[derive(IntoElement)]
pub struct DiffPage {
    values: DiffOptionValues,
    ws: Entity<Workspace>,
}

impl DiffPage {
    pub fn new(values: DiffOptionValues, ws: Entity<Workspace>) -> Self {
        Self { values, ws }
    }

    fn display_tabs(&self) -> impl IntoElement {
        let ws = self.ws.clone();
        div().w(px(300.0)).pb(px(6.0)).child(
            Tabs::new("settings-diff-display", usize::from(self.values.split))
                .tab("Unified", None)
                .tab("Split", None)
                .on_select(move |ix, _, cx| ws.update(cx, |this, cx| this.set_diff_split(ix == 1, cx))),
        )
    }
}

impl RenderOnce for DiffPage {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let (wrap, hide_ws) = (self.values.wrap, self.values.hide_whitespace);
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(div().font_weight(gpui::FontWeight::MEDIUM).child("Display"))
            .child(self.display_tabs())
            .child(CheckOptionRow::new("settings-diff-wrap", "Wrap long lines", wrap, self.ws.clone(), move |this, cx| {
                this.set_diff_wrap(!wrap, cx)
            }))
            .child(CheckOptionRow::new("settings-diff-ws", "Hide whitespace changes", hide_ws, self.ws, move |this, cx| {
                this.set_ignore_whitespace(!hide_ws, cx)
            }))
            .child(
                div()
                    .pt(px(6.0))
                    .text_size(px(12.0))
                    .text_color(theme.text_faint)
                    .child("These apply to every repository."),
            )
    }
}
