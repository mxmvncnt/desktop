//! One row of the changes list. Hot (virtualized): a plain fn, not a component.

use gd_git::{FileChange, FileStatus};
use gd_ui::{Checkbox, Icon, Theme, icon, pressable, theme::ROW_HEIGHT};
use gpui::{
    AnyElement, App, ColorExt as _, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};

use crate::repo::RepoModel;
use crate::shared::file_status_color::status_color;
use crate::shared::split_path::split_path;
use crate::workspace::Workspace;

pub(super) fn change_row(
    ix: usize,
    change: FileChange,
    repo: &Entity<RepoModel>,
    ws: &Entity<Workspace>,
    cx: &mut App,
) -> AnyElement {
    let r = repo.read(cx);
    let selected = r.selected_change == Some(ix);
    let check = r.file_check_state(&change.path);
    let (repo_click, repo_check, ws_row) = (repo.clone(), repo.clone(), ws.clone());
    let path = change.path.clone();
    pressable(("change", ix), selected, cx)
        .flex()
        .items_center()
        .gap(px(10.0))
        .h(px(ROW_HEIGHT))
        .w_full()
        .px(px(8.0))
        .rounded(px(6.0))
        .child(Checkbox::new(("check", ix), check).on_click(move |_, _, cx| {
            repo_check.update(cx, |r, cx| r.toggle_included(&path, cx))
        }))
        .child(file_name(&change.path, selected, cx))
        .child(if change.status == FileStatus::Conflicted { conflict_pill(cx) } else { status_badge(change.status, cx) })
        .on_mouse_down(MouseButton::Right, move |e, w, cx| {
            let pos = e.position;
            ws_row.update(cx, |this, cx| this.open_file_menu(ix, pos, w, cx))
        })
        .on_click(move |_, _, cx| repo_click.update(cx, |r, cx| r.select_change(ix, cx)))
        .into_any_element()
}

/// Dim directory, bright file name.
fn file_name(path: &str, selected: bool, cx: &App) -> impl IntoElement {
    let theme = Theme::of(cx).clone();
    let (dir, name) = split_path(path);
    div()
        .flex()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_size(px(12.5))
        .child(div().flex_shrink(1.0).min_w_0().truncate().text_color(theme.text_faint).child(dir.to_string()))
        .child(
            div()
                .flex_none()
                .text_color(if selected { theme.text } else { theme.text_muted })
                .child(name.to_string()),
        )
}

/// Conflict pill: red wash + warning glyph.
fn conflict_pill(cx: &App) -> AnyElement {
    let theme = Theme::of(cx).clone();
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(3.0))
        .px(px(6.0))
        .h(px(18.0))
        .rounded_full()
        .bg(theme.danger.opacity(0.14))
        .text_size(px(10.5))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme.danger)
        .child(icon(Icon::Warning).size(px(11.0)).text_color(theme.danger))
        .child("Conflict")
        .into_any_element()
}

fn status_badge(status: FileStatus, cx: &App) -> AnyElement {
    div()
        .flex_none()
        .w(px(14.0))
        .text_size(px(13.0))
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(status_color(status, cx))
        .child(status.badge())
        .into_any_element()
}
