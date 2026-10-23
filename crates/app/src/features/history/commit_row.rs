//! One commit of the history list: summary, tags, push arrow, author and time.

use gd_git::Commit;
use gd_ui::{Icon, Theme, icon, pressable};
use gpui::{App, Entity, IntoElement, MouseButton, RenderOnce, Window, div, prelude::*, px};

use super::state::ClickKind;
use crate::repo::RepoModel;
use crate::shared::commit_authors::authors_text;
use crate::shared::relative_time::relative_time;
use crate::workspace::Workspace;

pub const COMMIT_ROW_HEIGHT: f32 = 50.0;
/// Tag pills shown on a row before collapsing the rest into "+N".
const MAX_TAG_PILLS: usize = 2;

#[derive(IntoElement)]
pub struct CommitRow {
    ix: usize,
    commit: Commit,
    selected: bool,
    unpushed: bool,
    tags: Vec<String>,
    repo: Entity<RepoModel>,
    ws: Entity<Workspace>,
}

impl CommitRow {
    /// Reads the row's display state from the model; `None` past the end.
    pub fn from_model(ix: usize, repo: &Entity<RepoModel>, ws: &Entity<Workspace>, cx: &App) -> Option<Self> {
        let r = repo.read(cx);
        let commit = r.commits.get(ix)?.clone();
        Some(Self {
            ix,
            selected: r.commit_selected(ix),
            // No remote: nothing to push to, so no "will be pushed" arrow.
            unpushed: !r.history.all_local && r.is_unpushed(&commit.sha),
            tags: r.history.tags.get(&commit.sha).cloned().unwrap_or_default(),
            commit,
            repo: repo.clone(),
            ws: ws.clone(),
        })
    }
}

impl RenderOnce for CommitRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let (ix, repo, ws) = (self.ix, self.repo.clone(), self.ws.clone());
        pressable(("commit", ix), self.selected, cx)
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(3.0))
            .h(px(COMMIT_ROW_HEIGHT))
            .w_full()
            .px(px(10.0))
            .rounded(px(6.0))
            .child(self.title_line(&theme))
            .child(
                div()
                    .truncate()
                    .text_size(px(11.5))
                    .text_color(theme.text_faint)
                    .child(format!("{} • {}", authors_text(&self.commit), relative_time(self.commit.time))),
            )
            .on_mouse_down(MouseButton::Right, move |e, w, cx| {
                let pos = e.position;
                ws.update(cx, |this, cx| this.open_commit_menu(ix, pos, w, cx))
            })
            .on_click(move |ev, _, cx| {
                if ev.standard_click() {
                    repo.update(cx, |r, cx| r.click_commit(ix, click_kind(&ev.modifiers()), cx))
                }
            })
    }
}

impl CommitRow {
    fn title_line(&self, theme: &Theme) -> impl IntoElement + use<> {
        div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(13.0))
                    .text_color(if self.selected { theme.text } else { theme.text_muted })
                    .child(self.commit.summary.clone()),
            )
            .children(tag_pills(&self.tags, theme))
            .when(self.unpushed, |d| d.child(unpushed_arrow(self.ix, theme)))
    }
}

fn click_kind(m: &gpui::Modifiers) -> ClickKind {
    if m.shift {
        ClickKind::Range
    } else if m.secondary() {
        ClickKind::Toggle
    } else {
        ClickKind::Plain
    }
}

fn unpushed_arrow(ix: usize, theme: &Theme) -> impl IntoElement + use<> {
    div()
        .id(("unpushed", ix))
        .flex_none()
        .child(icon(Icon::ArrowUp).size(px(13.0)).text_color(theme.text_faint))
        .tooltip(gd_ui::tooltip("This commit hasn't been pushed to the remote repository yet"))
}

/// Up to [`MAX_TAG_PILLS`] tag pills (+N for the rest).
fn tag_pills(tags: &[String], theme: &Theme) -> Vec<gpui::AnyElement> {
    let pill = |text: String| {
        div()
            .flex_none()
            .max_w(px(90.0))
            .truncate()
            .px(px(6.0))
            .py(px(1.0))
            .rounded(px(5.0))
            .bg(gd_ui::theme::ink(0.07))
            .text_size(px(10.5))
            .text_color(theme.text_muted)
            .child(text)
            .into_any_element()
    };
    let mut v: Vec<_> = tags.iter().take(MAX_TAG_PILLS).cloned().map(pill).collect();
    if tags.len() > MAX_TAG_PILLS {
        v.push(pill(format!("+{}", tags.len() - MAX_TAG_PILLS)));
    }
    v
}
