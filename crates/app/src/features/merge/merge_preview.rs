//! Desktop-style preview sentence under the merge dialog's list, plus the
//! expected-conflicts note.

use gd_git::MergePreview;
use gd_ui::{Icon, Theme, icon};
use gpui::{App, Hsla, IntoElement, ParentElement, RenderOnce, Styled, Window, div, prelude::*, px};

use super::state::MergeMode;
use crate::shared::plural::plural;

#[derive(IntoElement)]
pub struct MergePreviewLine {
    mode: MergeMode,
    current: String,
    preview: Option<MergePreview>,
    loading: bool,
    has_candidates: bool,
}

/// A piece of the sentence; `Bold` is a branch name or a count.
enum Part {
    Plain(String),
    Bold(String),
}

impl MergePreviewLine {
    pub fn new(
        mode: MergeMode,
        current: String,
        preview: Option<MergePreview>,
        loading: bool,
        has_candidates: bool,
    ) -> Self {
        Self { mode, current, preview, loading, has_candidates }
    }
}

impl RenderOnce for MergePreviewLine {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let line = div().flex().flex_wrap().items_center().gap_x(px(4.0)).text_size(px(12.5));
        let Some(p) = &self.preview else {
            return self.placeholder(line, &theme).into_any_element();
        };
        let sentence = sentence(self.mode, &self.current, p).into_iter().fold(line, |l, part| l.child(part_el(part, &theme)));
        div()
            .flex()
            .flex_col()
            .gap(px(3.0))
            .child(sentence)
            .children(self.conflict_note(p, &theme))
            .into_any_element()
    }
}

impl MergePreviewLine {
    /// Shown while loading or when there is nothing to preview.
    fn placeholder(&self, line: gpui::Div, theme: &Theme) -> gpui::Div {
        let text = if self.loading {
            "Checking the branch…"
        } else if self.has_candidates {
            ""
        } else {
            "There are no other branches to merge."
        };
        line.text_color(theme.text_faint).child(text.to_string())
    }

    fn conflict_note(&self, p: &MergePreview, theme: &Theme) -> Option<gpui::Div> {
        if p.ahead == 0 {
            return None;
        }
        match &p.conflicts {
            Some(c) if !c.is_empty() => Some(note(
                Icon::Warning,
                theme.warning,
                format!("{} conflicted file{} expected. You'll resolve them next.", c.len(), plural(c.len() as u32)),
            )),
            Some(_) if p.behind > 0 && self.mode != MergeMode::Rebase => {
                Some(note(Icon::Check, theme.success, "No conflicts expected.".to_string()))
            }
            _ => None,
        }
    }
}

fn note(ic: Icon, color: Hsla, text: String) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .text_size(px(12.0))
        .text_color(color)
        .child(icon(ic).size(px(13.0)).text_color(color))
        .child(text)
}

fn part_el(part: Part, theme: &Theme) -> gpui::Div {
    let (text, bold) = match part {
        Part::Plain(t) => (t, false),
        Part::Bold(t) => (t, true),
    };
    div()
        .text_color(if bold { theme.text } else { theme.text_muted })
        .when(bold, |d| d.font_weight(gpui::FontWeight::MEDIUM))
        .child(text)
}

fn sentence(mode: MergeMode, current: &str, p: &MergePreview) -> Vec<Part> {
    let s = |t: &str| Part::Plain(t.to_string());
    let b = |t: String| Part::Bold(t);
    let commits = |n: u32| format!("{n} commit{}", plural(n));
    match (mode, p.ahead) {
        (MergeMode::Rebase, 0) => vec![b(current.to_string()), s("is already based on"), b(format!("{}.", p.branch))],
        (_, 0) => vec![s("This branch is up to date with"), b(format!("{current}."))],
        (MergeMode::Merge, n) => vec![
            s("This will merge"),
            b(commits(n)),
            s("from"),
            b(p.branch.clone()),
            s("into"),
            b(format!("{current}.")),
        ],
        (MergeMode::Squash, n) => vec![
            s("This will squash"),
            b(commits(n)),
            s("from"),
            b(p.branch.clone()),
            s("into one commit on"),
            b(format!("{current}.")),
        ],
        (MergeMode::Rebase, _) if p.behind == 0 => {
            vec![s("This will fast-forward"), b(current.to_string()), s("to"), b(format!("{}.", p.branch))]
        }
        (MergeMode::Rebase, _) => vec![
            s("This will replay"),
            b(commits(p.behind)),
            s("of"),
            b(current.to_string()),
            s("on top of"),
            b(format!("{}.", p.branch)),
        ],
    }
}
