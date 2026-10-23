//! Strip above the diff of a conflicted file: marker count + resolve actions.

use std::path::PathBuf;
use std::sync::Arc;

use gd_git::{Diff, DiffLineKind, FileChange, InProgress};
use gd_ui::{Button, Icon, Theme, icon};
use gpui::{App, ColorExt as _, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};

use super::state::Resolve;
use crate::repo::RepoModel;
use crate::shared::file_url::file_url;
use crate::shared::plural::plural;

#[derive(IntoElement)]
pub struct ConflictHint {
    path: String,
    markers: usize,
    /// "ours: main · theirs: feature", when known.
    sides: Option<String>,
    busy: bool,
    root: PathBuf,
    repo: Entity<RepoModel>,
}

impl ConflictHint {
    pub fn new(change: &FileChange, diff: Option<&Arc<Diff>>, repo: Entity<RepoModel>, cx: &App) -> Self {
        let r = repo.read(cx);
        let sides = match &r.merge.in_progress {
            Some(InProgress::Merge { branch: Some(b), .. }) => Some(format!("ours: {} · theirs: {b}", r.branch_name())),
            _ => None,
        };
        let (busy, root) = (r.merge.busy.is_some(), r.repo.root().to_path_buf());
        Self { path: change.path.clone(), markers: count_markers(diff), sides, busy, root, repo }
    }
}

impl RenderOnce for ConflictHint {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let abs = self.root.join(&self.path);
        div()
            .flex()
            .flex_none()
            .items_center()
            .flex_wrap()
            .gap(px(10.0))
            .px(px(16.0))
            .py(px(8.0))
            .bg(theme.danger.opacity(0.07))
            .border_b_1()
            .border_color(theme.danger.opacity(0.2))
            .child(icon(Icon::Warning).size(px(15.0)).text_color(theme.danger))
            .child(self.text(&theme))
            .child(self.resolve_button("hint-ours", "Use ours", Resolve::Ours))
            .child(self.resolve_button("hint-theirs", "Use theirs", Resolve::Theirs))
            .child(
                Button::new("hint-editor")
                    .compact()
                    .icon(Icon::Document)
                    .label("Open in editor")
                    .on_click(move |_, _, cx| cx.open_url(&file_url(&abs))),
            )
            .child(self.resolve_button("hint-resolved", "Mark as resolved", Resolve::Marked).primary())
    }
}

impl ConflictHint {
    fn message(&self) -> String {
        if self.markers > 0 {
            format!(
                "{} conflict{} in this file. Edit the markers in your editor, or keep one side.",
                self.markers,
                plural(self.markers as u32)
            )
        } else {
            "This file was changed on both sides. Choose which version to keep.".to_string()
        }
    }

    fn text(&self, theme: &Theme) -> impl IntoElement + use<> {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(260.0))
            .child(div().text_size(px(12.5)).text_color(theme.text).child(self.message()))
            .children(
                self.sides
                    .clone()
                    .map(|s| div().text_size(px(11.5)).text_color(theme.text_faint).child(s)),
            )
    }

    fn resolve_button(&self, id: &'static str, label: &'static str, how: Resolve) -> Button {
        let (repo, path) = (self.repo.clone(), self.path.clone());
        Button::new(id)
            .compact()
            .label(label)
            .disabled(self.busy)
            .on_click(move |_, _, cx| repo.update(cx, |r, cx| r.resolve_file(path.clone(), how, cx)))
    }
}

fn count_markers(diff: Option<&Arc<Diff>>) -> usize {
    diff.map_or(0, |d| {
        d.lines.iter().filter(|l| l.kind == DiffLineKind::Add && l.text.starts_with("<<<<<<<")).count()
    })
}
