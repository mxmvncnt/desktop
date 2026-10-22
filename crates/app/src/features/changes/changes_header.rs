use gd_ui::{CheckState, Checkbox, Theme};
use gpui::{
    App, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement, RenderOnce, Styled, Window, div, px,
};

use super::state::SelectionSummary;
use crate::repo::RepoModel;
use crate::workspace::Workspace;

/// "N changed files" with the select-all checkbox; right-click opens the header menu.
#[derive(IntoElement)]
pub struct ChangesHeader {
    selection: SelectionSummary,
    repo: Entity<RepoModel>,
    ws: Entity<Workspace>,
}

impl ChangesHeader {
    pub fn new(selection: SelectionSummary, repo: Entity<RepoModel>, ws: Entity<Workspace>) -> Self {
        Self { selection, repo, ws }
    }
}

impl RenderOnce for ChangesHeader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let SelectionSummary { total, all, .. } = self.selection;
        let (repo, ws) = (self.repo, self.ws);
        div()
            .id("changes-header")
            .on_mouse_down(MouseButton::Right, move |e, w, cx| {
                let pos = e.position;
                ws.update(cx, |this, cx| this.open_header_menu(pos, w, cx))
            })
            .flex()
            .flex_none()
            .items_center()
            .gap(px(10.0))
            .h(px(36.0))
            .px(px(14.0))
            .text_size(px(12.0))
            .text_color(theme.text_muted)
            .child(Checkbox::new("all-files", all).on_click(move |_, _, cx| {
                repo.update(cx, |r, cx| r.set_all_included(all != CheckState::Checked, cx))
            }))
            .child(format!("{total} changed file{}", if total == 1 { "" } else { "s" }))
    }
}
