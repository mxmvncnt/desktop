//! One row of the sidebar stash list: title, branch, relative time.

use gd_git::Stash;
use gd_ui::{Icon, Theme, badge, icon, pressable};
use gpui::{App, Entity, IntoElement, MouseButton, RenderOnce, Window, div, prelude::*, px};

use crate::repo::RepoModel;
use crate::shared::relative_time::relative_time;
use crate::workspace::Workspace;

pub const STASH_ROW_HEIGHT: f32 = 46.0;

#[derive(IntoElement)]
pub struct StashRow {
    stash: Stash,
    selected: bool,
    /// Made on the checked-out branch.
    here: bool,
    /// "All branches" filter is on (shows the "this branch" badge).
    all_branches: bool,
    repo: Entity<RepoModel>,
    ws: Entity<Workspace>,
}

impl StashRow {
    pub fn new(
        stash: Stash,
        selected: bool,
        here: bool,
        all_branches: bool,
        repo: Entity<RepoModel>,
        ws: Entity<Workspace>,
    ) -> Self {
        Self { stash, selected, here, all_branches, repo, ws }
    }
}

impl RenderOnce for StashRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let (here, selected) = (self.here, self.selected);
        let (stash, menu_stash) = (self.stash.clone(), self.stash.clone());
        let (repo, ws) = (self.repo, self.ws);
        pressable(("stash", stash.index), selected, cx)
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(3.0))
            .h(px(STASH_ROW_HEIGHT))
            .w_full()
            .px(px(10.0))
            .rounded(px(6.0))
            .child(
                div()
                    .truncate()
                    .text_size(px(13.0))
                    .text_color(if here || selected { theme.text } else { theme.text_muted })
                    .child(stash.title()),
            )
            .child(meta_line(&stash, here, self.all_branches, &theme, cx))
            .on_mouse_down(MouseButton::Right, move |e, w, cx| {
                let (pos, s) = (e.position, menu_stash.clone());
                ws.update(cx, |this, cx| this.open_stash_menu(s, pos, w, cx))
            })
            .on_click(move |_, _, cx| {
                let sha = stash.sha.clone();
                repo.update(cx, |r, cx| r.select_stash(sha, cx))
            })
    }
}

/// Second line: branch name, relative time, optional "this branch" badge.
fn meta_line(s: &Stash, here: bool, all: bool, theme: &Theme, cx: &App) -> impl IntoElement + use<> {
    let accent = theme.accent;
    div()
        .flex()
        .items_center()
        .gap(px(5.0))
        .text_size(px(11.5))
        .text_color(theme.text_faint)
        .child(icon(Icon::GitBranch).size(px(11.0)).text_color(if here { theme.accent } else { theme.text_faint }))
        .child(
            div()
                .flex_shrink(1.0)
                .min_w_0()
                .truncate()
                .text_color(if here { theme.text_muted } else { theme.text_faint })
                .child(s.branch.clone().unwrap_or_else(|| "(no branch)".into())),
        )
        .child(div().flex_none().child(format!("• {}", relative_time(s.time))))
        .when(here && all, |d| {
            d.child(div().flex_1())
                .child(badge("this branch", cx).h(px(16.0)).text_size(px(10.5)).text_color(accent))
        })
}
