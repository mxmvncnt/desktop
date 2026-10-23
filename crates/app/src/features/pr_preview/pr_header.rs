//! Header of the pull-request preview: summary line, base selector,
//! publish hint and the "Create pull request" button.

use gd_git::{compare_url, host_of};
use gd_ui::{Button, Icon, Theme, badge, icon};
use gpui::{App, Entity, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px};

use super::pr_base_menu::PrBaseMenu;
use crate::repo::RepoModel;
use crate::shared::plural::plural;

#[derive(IntoElement)]
pub struct PrHeader {
    repo: Entity<RepoModel>,
}

impl PrHeader {
    pub fn new(repo: Entity<RepoModel>) -> Self {
        Self { repo }
    }
}

impl RenderOnce for PrHeader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let hint = self.hint(cx);
        div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(8.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(theme.border)
            .child(self.title_row(cx))
            .child(self.summary_row(&theme, cx))
            .children(hint.map(|(text, color)| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .text_size(px(12.0))
                    .text_color(color(&theme))
                    .child(icon(Icon::Info).size(px(13.0)))
                    .child(text)
            }))
    }
}

impl PrHeader {
    fn title_row(&self, cx: &App) -> impl IntoElement + use<> {
        let close = self.repo.clone();
        let url = self.url(cx);
        let host = self.repo.read(cx).pr.remote_url.as_deref().and_then(host_of);
        let label = host.map_or("Create pull request".to_string(), |h| match h {
            gd_git::Host::GitLab => "Create merge request".to_string(),
            _ => "Create pull request".to_string(),
        });
        div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .child(icon(Icon::Fork).size(px(16.0)))
            .child(div().flex_1().text_size(px(15.0)).font_weight(gpui::FontWeight::SEMIBOLD).child("Preview pull request"))
            .child(
                Button::new("pr-create")
                    .primary()
                    .icon(Icon::Globe)
                    .label(label)
                    .disabled(url.is_none())
                    .on_click(move |_, _, cx| {
                        if let Some(u) = &url {
                            cx.open_url(u);
                        }
                    }),
            )
            .child(
                Button::new("pr-close")
                    .ghost()
                    .icon(Icon::Close)
                    .on_click(move |_, _, cx| close.update(cx, |r, cx| r.close_pr_preview(cx))),
            )
    }

    fn summary_row(&self, theme: &Theme, cx: &App) -> impl IntoElement + use<> {
        let r = self.repo.read(cx);
        let pr = &r.pr;
        let base = pr.base.clone().unwrap_or_else(|| "(no base)".into());
        let toggle = self.repo.clone();
        let menu = PrBaseMenu::new(&self.repo, cx);
        let summary: SharedString = match (&pr.comparison, pr.loading) {
            (Some(c), _) => {
                let files = c.files.len();
                format!("{} commit{} • {} file{} changed", c.ahead, plural(c.ahead), files, plural(files as u32)).into()
            }
            (None, true) => "Comparing…".into(),
            (None, false) => "".into(),
        };
        let behind = pr.comparison.as_ref().map(|c| c.behind).filter(|&b| b > 0);
        div()
            .flex()
            .items_center()
            .flex_wrap()
            .gap(px(8.0))
            .text_color(theme.text_muted)
            .child(summary)
            .child("from")
            .child(badge(r.branch_name(), cx).text_color(theme.accent))
            .child("into")
            .child(
                div().relative().child(
                    Button::new("pr-base")
                        .compact()
                        .icon(Icon::GitBranch)
                        .label(base)
                        .on_click(move |_, _, cx| toggle.update(cx, |r, cx| r.toggle_pr_base_menu(cx))),
                )
                .children(menu),
            )
            .children(behind.map(|b| {
                div()
                    .text_size(px(12.0))
                    .text_color(theme.text_faint)
                    .child(format!("({b} commit{} behind the base)", plural(b)))
            }))
    }

    /// Warning / info line and the color to draw it in.
    fn hint(&self, cx: &App) -> Option<(String, fn(&Theme) -> gpui::Hsla)> {
        let r = self.repo.read(cx);
        let warn: fn(&Theme) -> gpui::Hsla = |t| t.warning;
        let faint: fn(&Theme) -> gpui::Hsla = |t| t.text_faint;
        if let Some(e) = &r.pr.error {
            return Some((e.clone(), warn));
        }
        let status = r.status.as_ref()?;
        if status.branch.is_none() {
            return Some(("HEAD is detached: check out a branch to open a pull request.".into(), warn));
        }
        if status.upstream.is_none() {
            return Some(("This branch is not published yet. Publish it before creating the pull request.".into(), warn));
        }
        if status.ahead_behind.ahead > 0 {
            let n = status.ahead_behind.ahead;
            return Some((format!("{n} local commit{} not pushed yet and missing from the pull request.", plural(n)), warn));
        }
        match r.pr.remote_url.as_deref() {
            None => Some(("No remote configured: the pull request can't be opened in a browser.".into(), faint)),
            Some(u) if host_of(u).is_none() => {
                Some(("Only GitHub, GitLab and Bitbucket remotes can open a pull request from here.".into(), faint))
            }
            _ => None,
        }
    }

    /// Browser URL for the button; `None` disables it.
    fn url(&self, cx: &App) -> Option<String> {
        let r = self.repo.read(cx);
        let remote = r.pr.remote_url.as_deref()?;
        let base = r.pr_base_on_remote(r.pr.base.as_deref()?);
        compare_url(remote, &base, &r.pr_head_name())
    }
}
