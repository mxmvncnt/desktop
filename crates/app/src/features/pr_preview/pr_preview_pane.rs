//! Main pane while the pull-request preview is open: header, file list and
//! read-only diff. An `impl Workspace` method because the diff renderer owns
//! scroll state (`&mut Workspace`).

use gd_ui::{Theme, vdivider};
use gpui::{AnyElement, Context, Entity, IntoElement, SharedString, div, prelude::*, px};

use super::pr_file_list::PrFileList;
use super::pr_header::PrHeader;
use crate::features::diff::DiffPane;
use crate::repo::RepoModel;
use crate::workspace::{NavList, Workspace, nav_focus};

impl Workspace {
    pub fn render_pr_preview(&mut self, repo: &Entity<RepoModel>, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::of(cx).clone();
        let r = repo.read(cx);
        let pr = &r.pr;
        let header = PrHeader::new(repo.clone());
        let body = match &pr.comparison {
            Some(c) if !c.files.is_empty() => {
                let file_key = pr.selected_file.and_then(|ix| c.files.get(ix)).map(|f| f.path.clone()).unwrap_or_default();
                let key = SharedString::from(format!("pr-diff-{}-{file_key}", c.merge_base));
                let (diff, hl) = (pr.diff.clone(), pr.hl.clone());
                let file_list = nav_focus(div(), NavList::PrFiles, cx.entity())
                    .flex()
                    .flex_col()
                    .flex_none()
                    .w(px(260.0))
                    .py(px(4.0))
                    .child(PrFileList::new(repo.clone()));
                let diff = self.render_diff(diff, hl, key, DiffPane::ReadOnly, cx);
                div().flex().flex_1().min_h_0().child(file_list).child(vdivider(cx)).child(diff).into_any_element()
            }
            Some(_) => empty(&theme, "No changes: nothing to compare against the base branch."),
            None if pr.loading => empty(&theme, "Comparing branches…"),
            None if pr.error.is_some() => empty(&theme, "The branches can't be compared."),
            None => empty(&theme, "No base branch found: pick one above."),
        };
        div().flex().flex_col().flex_1().min_w_0().child(header).child(body).into_any_element()
    }
}

fn empty(theme: &Theme, text: &'static str) -> AnyElement {
    div()
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .text_color(theme.text_muted)
        .child(text)
        .into_any_element()
}

impl Workspace {
    /// Menu / shortcut entry point; needs an open repository.
    pub fn open_pr_preview(&mut self, cx: &mut Context<Self>) {
        self.popover = None;
        if let Some(repo) = self.repo.clone() {
            repo.update(cx, |r, cx| r.open_pr_preview(cx));
        }
    }

    /// Esc: collapse the base dropdown, else leave the preview. False if not open.
    pub(crate) fn close_pr_preview_key(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(repo) = self.repo.clone().filter(|r| r.read(cx).pr.open) else { return false };
        repo.update(cx, |r, cx| if r.pr.base_menu { r.toggle_pr_base_menu(cx) } else { r.close_pr_preview(cx) });
        true
    }
}
