//! Banner for a paused merge / rebase / cherry-pick / revert (Continue +
//! Abort), and the hint strip above the diff of a conflicted file.

use std::sync::Arc;

use gd_git::{Diff, DiffLineKind, FileChange, InProgress};
use gd_ui::{Button, Icon, Theme, icon, motion};
use gpui::{
    ColorExt as _, Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px,
};

use crate::merge_ops::Resolve;
use crate::workspace::{Workspace, plural};

/// `file:///a%20b/c` for `cx.open_url`.
pub fn file_url(path: &std::path::Path) -> String {
    let mut out = String::from("file://");
    for b in path.display().to_string().bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// (title, detail) lines for the banner.
fn describe(op: &InProgress, current: &str, conflicts: usize) -> (String, String) {
    let todo = if conflicts > 0 {
        format!(
            "{conflicts} conflicted file{} remaining. Resolve {}, then continue.",
            plural(conflicts as u32),
            if conflicts == 1 { "it" } else { "them" }
        )
    } else {
        "All conflicts are resolved. Continue to finish.".to_string()
    };
    match op {
        InProgress::Merge { branch, .. } => (
            match branch {
                Some(b) => format!("Merging {b} into {current}"),
                None => format!("Merging into {current}"),
            },
            todo,
        ),
        InProgress::Squash => (format!("Squash merging into {current}"), todo),
        InProgress::Rebase { branch, onto, step, total } => (
            format!(
                "Rebasing {} onto {}",
                branch.as_deref().unwrap_or(current),
                onto.as_deref().unwrap_or("…")
            ),
            if *total > 0 {
                format!("Commit {step} of {total}. {todo}")
            } else {
                todo
            },
        ),
        InProgress::CherryPick { sha, subject } => (format!("Cherry-picking {sha} {subject}"), todo),
        InProgress::Revert { sha, subject } => (format!("Reverting {sha} {subject}"), todo),
    }
}

fn continue_label(op: &InProgress) -> &'static str {
    match op {
        InProgress::Merge { .. } | InProgress::Squash => "Continue merge",
        InProgress::Rebase { .. } => "Continue rebase",
        InProgress::CherryPick { .. } => "Continue cherry-pick",
        InProgress::Revert { .. } => "Continue revert",
    }
}

impl Workspace {
    /// Full-width strip under the toolbar while an operation is paused.
    pub fn render_in_progress_banner(&mut self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let theme = Theme::of(cx).clone();
        let repo = self.repo.clone()?;
        let r = repo.read(cx);
        let op = r.merge.in_progress.clone()?;
        let conflicts = r.conflict_count();
        let busy = r.merge.busy;
        let (title, detail) = describe(&op, &r.branch_name(), conflicts);
        let detail = busy.map(String::from).unwrap_or(detail);
        let (r1, r2) = (repo.clone(), repo.clone());
        let can_continue = conflicts == 0 && busy.is_none();
        Some(motion::fade_quick(
            SharedString::from(format!("in-progress-{}", op.verb())),
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(12.0))
                .px(px(16.0))
                .py(px(9.0))
                .bg(theme.warning.opacity(0.07))
                .border_b_1()
                .border_color(theme.warning.opacity(0.22))
                .child(icon(Icon::Warning).size(px(16.0)).text_color(theme.warning))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .truncate()
                                .text_size(px(13.0))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(theme.text)
                                .child(title),
                        )
                        .child(div().truncate().text_size(px(12.0)).text_color(theme.text_muted).child(detail)),
                )
                .child(
                    Button::new("op-abort")
                        .compact()
                        .label("Abort")
                        .disabled(busy.is_some())
                        .on_click(move |_, _, cx| r1.update(cx, |r, cx| r.abort_operation(cx))),
                )
                .child(
                    Button::new("op-continue")
                        .compact()
                        .primary()
                        .icon(Icon::Check)
                        .label(continue_label(&op))
                        .disabled(!can_continue)
                        .on_click(move |_, _, cx| r2.update(cx, |r, cx| r.continue_operation(cx))),
                ),
        ))
    }

    /// Strip above the diff of a conflicted file: marker count + resolve actions.
    pub fn render_conflict_hint(
        &mut self,
        change: &FileChange,
        diff: Option<&Arc<Diff>>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = Theme::of(cx).clone();
        let Some(repo) = self.repo.clone() else { return div().into_any_element() };
        let markers = diff.map_or(0, |d| {
            d.lines
                .iter()
                .filter(|l| l.kind == DiffLineKind::Add && l.text.starts_with("<<<<<<<"))
                .count()
        });
        let r = repo.read(cx);
        let sides = match &r.merge.in_progress {
            Some(InProgress::Merge { branch: Some(b), .. }) => {
                Some(format!("ours: {} · theirs: {b}", r.branch_name()))
            }
            _ => None,
        };
        let busy = r.merge.busy.is_some();
        let root = r.repo.root().to_path_buf();
        let message = if markers > 0 {
            format!(
                "{markers} conflict{} in this file. Edit the markers in your editor, or keep one side.",
                plural(markers as u32)
            )
        } else {
            "This file was changed on both sides. Choose which version to keep.".to_string()
        };
        let path = change.path.clone();
        let btn = |id: &'static str, label: &'static str, how: Resolve| {
            let (repo, path) = (repo.clone(), path.clone());
            Button::new(id)
                .compact()
                .label(label)
                .disabled(busy)
                .on_click(move |_, _, cx| repo.update(cx, |r, cx| r.resolve_file(path.clone(), how, cx)))
        };
        let abs = root.join(&change.path);
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
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(260.0))
                    .child(div().text_size(px(12.5)).text_color(theme.text).child(message))
                    .children(sides.map(|s| {
                        div().text_size(px(11.5)).text_color(theme.text_faint).child(s)
                    })),
            )
            .child(btn("hint-ours", "Use ours", Resolve::Ours))
            .child(btn("hint-theirs", "Use theirs", Resolve::Theirs))
            .child(
                Button::new("hint-editor")
                    .compact()
                    .icon(Icon::Document)
                    .label("Open in editor")
                    .on_click(move |_, _, cx| cx.open_url(&file_url(&abs))),
            )
            .child(btn("hint-resolved", "Mark as resolved", Resolve::Marked).primary())
            .into_any_element()
    }
}
