//! Full-width banner under the toolbar while a merge / rebase / cherry-pick /
//! revert is paused (Continue + Abort).

use gd_git::InProgress;
use gd_ui::{Button, Icon, Theme, icon, motion};
use gpui::{App, ColorExt as _, Context, Entity, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px};

use crate::repo::RepoModel;
use crate::shared::plural::plural;
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct InProgressBanner {
    op: InProgress,
    title: String,
    detail: String,
    busy: bool,
    can_continue: bool,
    repo: Entity<RepoModel>,
}

impl InProgressBanner {
    /// `None` when no operation is paused.
    pub fn open(ws: &Workspace, cx: &mut Context<Workspace>) -> Option<Self> {
        let repo = ws.repo.clone()?;
        let r = repo.read(cx);
        let op = r.merge.in_progress.clone()?;
        let conflicts = r.conflict_count();
        let busy = r.merge.busy;
        let Text { title, detail } = describe(&op, &r.branch_name(), conflicts);
        Some(Self {
            op,
            title,
            detail: busy.map(String::from).unwrap_or(detail),
            busy: busy.is_some(),
            can_continue: conflicts == 0 && busy.is_none(),
            repo,
        })
    }
}

impl RenderOnce for InProgressBanner {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        motion::fade_quick(
            SharedString::from(format!("in-progress-{}", self.op.verb())),
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
                .child(self.text(&theme))
                .child(self.abort_button())
                .child(self.continue_button()),
        )
    }
}

impl InProgressBanner {
    fn text(&self, theme: &Theme) -> impl IntoElement + use<> {
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
                    .child(self.title.clone()),
            )
            .child(div().truncate().text_size(px(12.0)).text_color(theme.text_muted).child(self.detail.clone()))
    }

    fn abort_button(&self) -> Button {
        let repo = self.repo.clone();
        Button::new("op-abort")
            .compact()
            .label("Abort")
            .disabled(self.busy)
            .on_click(move |_, _, cx| repo.update(cx, |r, cx| r.abort_operation(cx)))
    }

    fn continue_button(&self) -> Button {
        let repo = self.repo.clone();
        Button::new("op-continue")
            .compact()
            .primary()
            .icon(Icon::Check)
            .label(continue_label(&self.op))
            .disabled(!self.can_continue)
            .on_click(move |_, _, cx| repo.update(cx, |r, cx| r.continue_operation(cx)))
    }
}

/// The two text lines of the banner.
struct Text {
    title: String,
    detail: String,
}

fn describe(op: &InProgress, current: &str, conflicts: usize) -> Text {
    let todo = todo_line(conflicts);
    let (title, detail) = match op {
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
            if *total > 0 { format!("Commit {step} of {total}. {todo}") } else { todo },
        ),
        InProgress::CherryPick { sha, subject } => (format!("Cherry-picking {sha} {subject}"), todo),
        InProgress::Revert { sha, subject } => (format!("Reverting {sha} {subject}"), todo),
    };
    Text { title, detail }
}

fn todo_line(conflicts: usize) -> String {
    if conflicts == 0 {
        return "All conflicts are resolved. Continue to finish.".to_string();
    }
    format!(
        "{conflicts} conflicted file{} remaining. Resolve {}, then continue.",
        plural(conflicts as u32),
        if conflicts == 1 { "it" } else { "them" }
    )
}

fn continue_label(op: &InProgress) -> &'static str {
    match op {
        InProgress::Merge { .. } | InProgress::Squash => "Continue merge",
        InProgress::Rebase { .. } => "Continue rebase",
        InProgress::CherryPick { .. } => "Continue cherry-pick",
        InProgress::Revert { .. } => "Continue revert",
    }
}
