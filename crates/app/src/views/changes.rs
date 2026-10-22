//! Sidebar (tabs + Changes list + commit box) and the Changes main pane.

use gd_ui::{
    Button, CheckState, Checkbox, Icon, Tabs, Theme, divider, icon, motion, pressable,
    theme::{ROW_HEIGHT, SIDEBAR_WIDTH},
};
use gpui::{
    Context, Entity, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, Window,
    div, prelude::*, px, uniform_list,
};

use super::{split_path, status_color};
use gd_git::FileStatus;
use gpui::ColorExt as _;
use crate::workspace::plural;
use crate::actions::LIST;
use crate::repo::{RepoModel, Tab};
use crate::workspace::{Workspace, text_field};

impl Workspace {
    pub fn render_sidebar(
        &mut self,
        repo: &Entity<RepoModel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = Theme::of(cx).clone();
        let r = repo.read(cx);
        let tab = r.tab;
        let n = r.changes().len();
        let repo_for_tabs = repo.clone();
        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .bg(theme.surface)
            .rounded_bl(px(self.corner_radius))
            .border_r_1()
            .border_color(theme.border)
            .child(
                div().p(px(8.0)).child(
                    Tabs::new("sidebar-tabs", if tab == Tab::Changes { 0 } else { 1 })
                        .tab("Changes", (n > 0).then(|| SharedString::from(n.to_string())))
                        .tab("History", None)
                        .on_select(move |ix, _, cx| {
                            repo_for_tabs.update(cx, |r, cx| {
                                r.set_tab(if ix == 0 { Tab::Changes } else { Tab::History }, cx)
                            })
                        }),
                ),
            )
            .child(divider(cx))
            .child(match tab {
                Tab::Changes => self.render_changes_list(repo, window, cx).into_any_element(),
                Tab::History => self.render_history_list(repo, cx).into_any_element(),
            })
    }

    fn render_changes_list(
        &mut self,
        repo: &Entity<RepoModel>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = Theme::of(cx).clone();
        let r = repo.read(cx);
        let n = r.changes().len();
        let included = n - r.changes().iter().filter(|c| r.excluded.contains(&c.path)).count();
        let all_state = match included {
            0 => CheckState::Unchecked,
            i if i == n && r.partial.is_empty() => CheckState::Checked,
            _ => CheckState::Mixed,
        };
        let branch = r.branch_name();
        let committing = r.committing;
        let amending = self.amending(cx);
        let summary_ok = !self.summary.read(cx).as_str().trim().is_empty();
        let conflicts = r.conflict_count();
        let merging = r.merge.in_progress.as_ref().is_some_and(|p| p.commit_finishes());
        // Amending may change only the message, so no files are needed.
        let can_commit = (included > 0 || amending)
            && summary_ok
            && !committing
            && conflicts == 0
            && r.merge.busy.is_none();
        let head_pushed = r.history_ops.push_state_loaded
            && r.commits.first().is_some_and(|c| !r.is_unpushed(&c.sha));
        let repo_h = repo.clone();
        let ws = cx.entity();
        let last_commit = r.undoable_commit().cloned().filter(|_| !amending);
        let list_repo = repo.clone();

        let ws_header = ws.clone();
        let header = div()
            .id("changes-header")
            .on_mouse_down(gpui::MouseButton::Right, move |e, w, cx| {
                let pos = e.position;
                ws_header.update(cx, |this, cx| this.open_header_menu(pos, w, cx))
            })
            .flex()
            .flex_none()
            .items_center()
            .gap(px(10.0))
            .h(px(36.0))
            .px(px(14.0))
            .text_size(px(12.0))
            .text_color(theme.text_muted)
            .child(Checkbox::new("all-files", all_state).on_click(move |_, _, cx| {
                repo_h.update(cx, |r, cx| r.set_all_included(all_state != CheckState::Checked, cx))
            }))
            .child(format!("{n} changed file{}", if n == 1 { "" } else { "s" }));

        let list = if n == 0 {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.text_faint)
                .text_size(px(12.0))
                .child("No local changes")
                .into_any_element()
        } else {
            uniform_list("changes", n, move |range, _window, cx| {
                let theme = Theme::of(cx).clone();
                let r = list_repo.read(cx);
                range
                    .filter_map(|ix| {
                        let change = r.changes().get(ix)?.clone();
                        let selected = r.selected_change == Some(ix);
                        let check = r.file_check_state(&change.path);
                        let (dir, name) = split_path(&change.path);
                        let (dir, name) = (dir.to_string(), name.to_string());
                        let repo_click = list_repo.clone();
                        let repo_check = list_repo.clone();
                        let path = change.path.clone();
                        let ws_row = ws.clone();
                        Some(
                            pressable(("change", ix), selected, cx)
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                .h(px(ROW_HEIGHT))
                                .w_full()
                                .px(px(8.0))
                                .rounded(px(6.0))
                                .child(
                                    Checkbox::new(("check", ix), check)
                                    .on_click(move |_, _, cx| {
                                        repo_check.update(cx, |r, cx| r.toggle_included(&path, cx))
                                    }),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_size(px(12.5))
                                        .child(
                                            div()
                                                .flex_shrink(1.0)
                                                .min_w_0()
                                                .truncate()
                                                .text_color(theme.text_faint)
                                                .child(dir),
                                        )
                                        .child(
                                            div()
                                                .flex_none()
                                                .text_color(if selected {
                                                    theme.text
                                                } else {
                                                    theme.text_muted
                                                })
                                                .child(name),
                                        ),
                                )
                                .child(if change.status == FileStatus::Conflicted {
                                    // Conflict pill: red wash + warning glyph.
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
                                } else {
                                    div()
                                        .flex_none()
                                        .w(px(14.0))
                                        .text_size(px(13.0))
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(status_color(change.status, cx))
                                        .child(change.status.badge())
                                        .into_any_element()
                                })
                                .on_mouse_down(gpui::MouseButton::Right, move |e, w, cx| {
                                    let pos = e.position;
                                    ws_row.update(cx, |this, cx| this.open_file_menu(ix, pos, w, cx))
                                })
                                .on_click(move |_, _, cx| {
                                    repo_click.update(cx, |r, cx| r.select_change(ix, cx))
                                }),
                        )
                    })
                    .collect()
            })
            .track_scroll(&self.changes_scroll)
            .px(px(6.0))
            .flex_1()
            .into_any_element()
        };

        let commit_label: SharedString = if amending {
            if committing { "Amending…".into() } else { "Amend last commit".into() }
        } else if committing {
            "Committing…".into()
        } else if conflicts > 0 {
            format!("Resolve {conflicts} conflict{} to commit", plural(conflicts as u32)).into()
        } else if merging {
            "Commit merge".into()
        } else {
            format!("Commit to {branch}").into()
        };

        let commit_box = div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(8.0))
            .p(px(12.0))
            .border_t_1()
            .border_color(theme.border)
            .children(amending.then(|| amend_notice(head_pushed, cx)))
            .child(text_field("summary", &self.summary, "Summary (required)", cx))
            .child(
                gpui_elements::editable_text::text_area("description")
                    .state(self.description.downgrade())
                    .placeholder("Description")
                    .placeholder_color(theme.text_faint)
                    .selection_color(theme.selection)
                    .caret_color(theme.caret)
                    .caret_blink_interval_500ms()
                    .w_full()
                    .h(px(88.0))
                    .px(px(10.0))
                    .py(px(8.0))
                    .rounded(px(gd_ui::theme::CONTROL_RADIUS + 1.0))
                    .bg(theme.input_bg)
                    .border_1()
                    .border_color(theme.border)
                    .text_size(px(13.0))
                    .text_color(theme.text)
                    .overflow_y_scroll(),
            )
            .child(
                Button::new("commit")
                    .primary()
                    .full_width()
                    .icon(if amending { Icon::Pen } else { Icon::Check })
                    .label(commit_label)
                    .disabled(!can_commit)
                    .on_click(|_, w, cx| w.dispatch_action(Box::new(crate::actions::Commit), cx)),
            );

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(header)
            .child(divider(cx))
            .child(div().key_context(LIST).flex().flex_col().flex_1().min_h_0().py(px(4.0)).child(list))
            .children(self.render_stash_panel(repo, cx))
            .children(last_commit.map(|c| undo_strip(c, cx)))
            .child(commit_box)
    }

    pub fn render_changes_main(
        &mut self,
        repo: &Entity<RepoModel>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = Theme::of(cx).clone();
        let r = repo.read(cx);
        if r.stashes.viewing {
            return self.render_stash_main(repo, cx);
        }
        let Some(change) = r.selected_change.and_then(|ix| r.changes().get(ix)).cloned() else {
            return self.render_no_changes(repo, cx).into_any_element();
        };
        let diff = r.change_diff.clone();
        let hl = r.change_hl.clone();
        let hint = (change.status == FileStatus::Conflicted)
            .then(|| self.render_conflict_hint(&change, diff.as_ref(), cx));
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(8.0))
                    .h(px(40.0))
                    .px(px(16.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(icon(Icon::Document).size(px(14.0)).text_color(theme.text_faint))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.5))
                            .text_color(theme.text_muted)
                            .child(match &change.old_path {
                                Some(old) => format!("{old} → {}", change.path),
                                None => change.path.clone(),
                            }),
                    )
                    .when_some(diff.as_ref(), |d, diff| {
                        d.child(
                            div()
                                .flex()
                                .gap(px(8.0))
                                .text_size(px(12.0))
                                .font_family(theme.font_mono.clone())
                                .child(div().text_color(theme.diff_add).child(format!("+{}", diff.added)))
                                .child(div().text_color(theme.diff_del).child(format!("−{}", diff.deleted))),
                        )
                    })
                    .child(self.diff_options_button(cx)),
            )
            .children(hint)
            .child(self.render_diff(
                diff,
                hl,
                SharedString::from(format!("change-diff-{}", change.path)),
                crate::views::diff::DiffPane::Changes(repo.clone()),
                cx,
            ))
            .into_any_element()
    }

    fn render_no_changes(&mut self, repo: &Entity<RepoModel>, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = Theme::of(cx).clone();
        let r = repo.read(cx);
        let name = r.repo.name();
        let root = r.repo.root().display().to_string();
        let ab = r.status.as_ref().map(|s| (s.ahead_behind, s.upstream.is_some()));
        let mut suggestions = div().flex().flex_col().gap(px(10.0)).w(px(460.0));
        if let Some((ab, true)) = ab {
            if ab.ahead > 0 {
                suggestions = suggestions.child(suggestion_card(
                    "push-suggestion",
                    Icon::ArrowUp,
                    "Push commits to the remote",
                    format!("You have {} local commit{} waiting to be pushed.", ab.ahead, crate::workspace::plural(ab.ahead)),
                    "Push",
                    |_, w: &mut Window, cx: &mut gpui::App| w.dispatch_action(Box::new(crate::actions::Push), cx),
                    cx,
                ));
            }
            if ab.behind > 0 {
                suggestions = suggestions.child(suggestion_card(
                    "pull-suggestion",
                    Icon::ArrowDown,
                    "Pull commits from the remote",
                    format!("The remote has {} commit{} you don't have.", ab.behind, crate::workspace::plural(ab.behind)),
                    "Pull",
                    |_, w: &mut Window, cx: &mut gpui::App| w.dispatch_action(Box::new(crate::actions::Pull), cx),
                    cx,
                ));
            }
        }
        suggestions = suggestions.child(suggestion_card(
            "open-folder",
            Icon::Folder,
            "View the files of your repository",
            root.clone(),
            "Open folder",
            move |_, _, cx: &mut gpui::App| cx.open_url(&format!("file://{root}")),
            cx,
        ));
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .child(motion::fade_in(
                ("no-changes", name.len()),
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(20.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(6.0))
                            .child(div().text_size(px(20.0)).font_weight(gpui::FontWeight::SEMIBOLD).child("No local changes"))
                            .child(
                                div()
                                    .text_color(theme.text_muted)
                                    .child(format!("There are no uncommitted changes in {name}.")),
                            ),
                    )
                    .child(suggestions),
            ))
    }
}

pub fn suggestion_card(
    id: &'static str,
    i: Icon,
    title: impl Into<SharedString>,
    body: impl Into<SharedString>,
    action: impl Into<SharedString>,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
    cx: &gpui::App,
) -> gpui::AnyElement {
    let theme = Theme::of(cx).clone();
    div()
        .flex()
        .items_center()
        .gap(px(14.0))
        .p(px(14.0))
        .rounded(px(gd_ui::theme::PANEL_RADIUS))
        .bg(theme.surface_card)
        .border_1()
        .border_color(theme.border)
        .child(
            div()
                .size(px(32.0))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.0))
                .bg(gd_ui::theme::ink(0.06))
                .child(icon(i).text_color(theme.text_muted)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(2.0))
                .child(div().font_weight(gpui::FontWeight::MEDIUM).child(title.into()))
                .child(div().text_size(px(12.0)).text_color(theme.text_faint).truncate().child(body.into())),
        )
        .child(Button::new(id).label(action).compact().on_click(on_click))
        .into_any_element()
}

/// Desktop's "Committed just now" strip with an Undo button.
fn undo_strip(c: crate::repo::LastCommit, cx: &mut Context<Workspace>) -> impl IntoElement + use<> {
    let theme = Theme::of(cx).clone();
    motion::fade_quick(
        ("undo-strip", c.time as usize),
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(10.0))
            .px(px(12.0))
            .py(px(8.0))
            .border_t_1()
            .border_color(theme.border)
            .bg(gd_ui::theme::ink(0.03))
            .child(icon(Icon::Check).size(px(14.0)).text_color(theme.success))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(theme.text_faint)
                            .child(format!("Committed {}", super::relative_time(c.time))),
                    )
                    .child(div().text_size(px(12.5)).truncate().child(c.summary)),
            )
            .child(
                Button::new("undo-commit")
                    .compact()
                    .icon(Icon::Restart)
                    .label("Undo")
                    .on_click(cx.listener(|this, _, _, cx| this.undo_commit(cx))),
            ),
    )
}

/// Desktop's "Your changes will modify your most recent commit" notice, with
/// a "Stop amending" link and a force-push warning for pushed commits.
fn amend_notice(pushed: bool, cx: &mut Context<Workspace>) -> impl IntoElement + use<> {
    let theme = Theme::of(cx).clone();
    motion::fade_quick(
        "amend-notice",
        div()
            .flex()
            .items_start()
            .gap(px(8.0))
            .p(px(10.0))
            .rounded(px(gd_ui::theme::CONTROL_RADIUS + 1.0))
            .bg(gd_ui::theme::ink(0.04))
            .border_1()
            .border_color(theme.border)
            .child(icon(if pushed { Icon::Warning } else { Icon::Info }).size(px(14.0)).text_color(if pushed {
                theme.warning
            } else {
                theme.text_muted
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .text_size(px(12.0))
                    .text_color(theme.text_muted)
                    .child("Your changes will modify your most recent commit.")
                    .when(pushed, |d| {
                        d.child(
                            div()
                                .text_color(theme.warning)
                                .child("It was already pushed: you'll need to force push afterwards."),
                        )
                    })
                    .child(
                        div()
                            .id("stop-amending")
                            .cursor_pointer()
                            .text_color(theme.text)
                            .underline()
                            .child("Stop amending")
                            .on_click(cx.listener(|this, _, _, cx| this.stop_amend(cx))),
                    ),
            ),
    )
}
