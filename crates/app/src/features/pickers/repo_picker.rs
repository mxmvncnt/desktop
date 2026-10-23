//! Repository list popover.

use gd_ui::{Button, Icon, Theme, icon, icon_button, pressable};
use gpui::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce, StatefulInteractiveElement,
    Styled, UniformListScrollHandle, Window, div, px, uniform_list,
};
use gpui_elements::editable_text::EditableTextState;

use super::picker_shell::{PICKER_ROW, PickerShell};
use crate::workspace::{Popover, Workspace};

/// One repository row of the list.
pub struct RepoEntry {
    /// Index into `settings.repositories`.
    pub ix: usize,
    pub name: String,
    pub parent: String,
    /// The repository currently open.
    pub current: bool,
}

#[derive(IntoElement)]
pub struct RepoPicker {
    entries: Vec<RepoEntry>,
    selected: usize,
    filter: Entity<EditableTextState>,
    scroll: UniformListScrollHandle,
    ws: Entity<Workspace>,
}

impl RepoPicker {
    pub fn new(
        entries: Vec<RepoEntry>,
        selected: usize,
        filter: Entity<EditableTextState>,
        scroll: UniformListScrollHandle,
        ws: Entity<Workspace>,
    ) -> Self {
        Self { entries, selected, filter, scroll, ws }
    }

    fn list(entries: Vec<RepoEntry>, selected: usize, scroll: &UniformListScrollHandle, ws: &Entity<Workspace>) -> AnyElement {
        let n = entries.len();
        let ws = ws.clone();
        uniform_list("repo-list", n, move |range, _w, cx| {
            range
                .filter_map(|i| Some(repo_row(entries.get(i)?, i == selected, &ws, cx)))
                .collect()
        })
        .track_scroll(scroll)
        .px(px(6.0))
        .size_full()
        .into_any_element()
    }

    fn footer(ws: &Entity<Workspace>) -> impl IntoElement + use<> {
        let (add, clone) = (ws.clone(), ws.clone());
        div()
            .flex()
            .gap(px(6.0))
            .w_full()
            .child(
                Button::new("add-repo")
                    .icon(Icon::Plus)
                    .label("Add existing…")
                    .compact()
                    .on_click(move |_, _, cx| add.update(cx, |this, cx| this.prompt_add_repository(cx))),
            )
            .child(
                Button::new("clone-repo")
                    .icon(Icon::Cloud)
                    .label("Clone…")
                    .compact()
                    .on_click(move |_, w, cx| {
                        clone.update(cx, |this, cx| this.toggle_popover(Popover::Clone, w, cx))
                    }),
            )
    }
}

impl RenderOnce for RepoPicker {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let n = self.entries.len();
        let footer = Self::footer(&self.ws);
        let list = Self::list(self.entries, self.selected, &self.scroll, &self.ws);
        let ws = self.ws;
        PickerShell::new("Repositories", "Filter repositories", self.filter, n, list, footer, ws)
            .on_enter(|this, _w, cx| this.enter_repo_picker(cx))
    }
}

/// Hot row (virtualized): a plain fn, not a component.
fn repo_row(e: &RepoEntry, selected: bool, ws: &Entity<Workspace>, cx: &mut App) -> AnyElement {
    let theme = Theme::of(cx).clone();
    let ix = e.ix;
    let (open, remove) = (ws.clone(), ws.clone());
    pressable(("repo", ix), selected, cx)
        .group("repo-row")
        .flex()
        .items_center()
        .gap(px(10.0))
        .h(px(PICKER_ROW))
        .w_full()
        .px(px(8.0))
        .rounded(px(6.0))
        .child(
            icon(if e.current { Icon::Check } else { Icon::Folder })
                .size(px(14.0))
                .text_color(if e.current { theme.text } else { theme.text_faint }),
        )
        .child(div().flex_none().text_color(theme.text).child(e.name.clone()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(11.5))
                .text_color(theme.text_faint)
                .child(e.parent.clone()),
        )
        .child(
            div().invisible().group_hover_style("repo-row", |s| s.visible()).child(
                icon_button(("remove-repo", ix), Icon::Close).compact().on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    remove.update(cx, |ws, cx| ws.remove_repository(ix, cx))
                }),
            ),
        )
        .on_click(move |_, _, cx| open.update(cx, |ws, cx| ws.open_repository(ix, cx)))
        .into_any_element()
}
