//! Multiple stashes: the sidebar panel (collapsible list with a branch
//! filter), the stash detail pane in the main area, the right-click menu rows,
//! and the stash / rename / discard / switch-branch dialogs.
//!
//! All state is `RepoModel::stashes` (model) and `Workspace::stash_ui`
//! (dialog). Dialogs use `gd_ui::Dialog::content` for their inputs/options.

use gd_git::{Branch, Stash};
use gd_ui::{Button, Dialog, Icon, Theme, badge, icon, pressable, vdivider};
use gpui::{
    App, ClipboardItem, Context, ElementId, Entity, Focusable, InteractiveElement, IntoElement, ParentElement,
    Pixels, Point, SharedString, Styled, Window, div, prelude::*, px, uniform_list,
};
use gpui_elements::editable_text::{EditableTextState, StringStorage, actions::Enter};

use super::menus::{MenuAction, MenuTarget};
use super::{relative_time, split_path, status_color};
use crate::actions::LIST;
use crate::repo::RepoModel;
use crate::workspace::{Workspace, plural, text_field};

const STASH_ROW: f32 = 46.0;
/// Rows visible before the panel's list scrolls.
const STASH_MAX_ROWS: usize = 4;

pub enum StashDialog {
    /// New stash. `selected_only` stashes just the checked files.
    Create { selected_only: bool },
    Rename(Stash),
    Drop(Stash),
    /// Checkout with uncommitted changes: `stash` = leave them behind.
    Switch { branch: Branch, stash: bool },
}

/// The one Workspace field for stash UI.
pub struct StashUi {
    pub dialog: Option<StashDialog>,
    pub input: Entity<EditableTextState>,
}

impl StashUi {
    pub fn new(cx: &mut App) -> Self {
        Self {
            dialog: None,
            input: cx.new(|cx| EditableTextState::new(StringStorage::default(), cx)),
        }
    }
}

// ---- menu rows --------------------------------------------------------------

/// `(action, label, icon, danger)` for the stash row menu; `None` = separator.
pub fn stash_menu_rows() -> Vec<Option<(MenuAction, &'static str, Icon, bool)>> {
    vec![
        Some((MenuAction::StashApply, "Apply (keep stash)", Icon::ArchiveUp, false)),
        Some((MenuAction::StashPop, "Pop (apply and remove)", Icon::Restart, false)),
        None,
        Some((MenuAction::StashRename, "Rename…", Icon::Pen, false)),
        Some((MenuAction::StashCopy, "Copy message", Icon::Copy, false)),
        None,
        Some((MenuAction::StashDrop, "Drop…", Icon::Trash, true)),
    ]
}

impl Workspace {
    /// Handles stash-related menu actions; returns false for other actions.
    pub fn run_stash_action(
        &mut self,
        action: MenuAction,
        target: &MenuTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let stash = match target {
            MenuTarget::Stash(s) => Some(s.clone()),
            _ => None,
        };
        match (action, stash) {
            (MenuAction::StashAll, _) => self.open_stash_create(false, window, cx),
            (MenuAction::StashSelected, _) => self.open_stash_create(true, window, cx),
            (MenuAction::StashApply, Some(s)) => self.stash_op(cx, move |r, cx| r.stash_apply(s.sha, cx)),
            (MenuAction::StashPop, Some(s)) => self.stash_op(cx, move |r, cx| r.stash_pop(s.sha, cx)),
            (MenuAction::StashDrop, Some(s)) => self.open_stash_dialog(StashDialog::Drop(s), window, cx),
            (MenuAction::StashRename, Some(s)) => {
                let m = s.message.clone();
                self.input_text(&m, cx);
                self.open_stash_dialog(StashDialog::Rename(s), window, cx)
            }
            (MenuAction::StashCopy, Some(s)) => cx.write_to_clipboard(ClipboardItem::new_string(s.title())),
            _ => return false,
        }
        true
    }

    fn stash_op(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut RepoModel, &mut Context<RepoModel>)) {
        if let Some(repo) = &self.repo {
            repo.update(cx, f);
        }
    }

    pub fn open_stash_menu(&mut self, stash: Stash, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        self.open_menu(position, MenuTarget::Stash(stash), window, cx);
    }

    fn input_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.stash_ui.input.update(cx, |s, cx| s.emplace(text, cx));
    }

    // ---- dialogs ----------------------------------------------------------------

    fn open_stash_dialog(&mut self, d: StashDialog, window: &mut Window, cx: &mut Context<Self>) {
        let wants_input = matches!(d, StashDialog::Create { .. } | StashDialog::Rename(_));
        self.menu = None;
        self.popover = None;
        self.dialog = None;
        self.stash_ui.dialog = Some(d);
        if wants_input {
            window.focus(&self.stash_ui.input.focus_handle(cx), cx);
        } else {
            self.focus(window, cx);
        }
        cx.notify();
    }

    /// Ctrl+Shift+S / the header menu. `selected_only` stashes the checked files.
    pub fn open_stash_create(&mut self, selected_only: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else { return };
        let r = repo.read(cx);
        if r.changes().is_empty() {
            return;
        }
        if selected_only && !r.has_included() {
            repo.update(cx, |r, cx| {
                r.notice = Some("Check at least one file to stash".into());
                cx.notify()
            });
            return;
        }
        self.input_text("", cx);
        self.open_stash_dialog(StashDialog::Create { selected_only }, window, cx);
    }

    /// Called when a checkout was requested with a dirty working tree.
    pub fn open_switch_dialog(&mut self, branch: Branch, cx: &mut Context<Self>) {
        self.menu = None;
        self.popover = None;
        self.dialog = None;
        // Desktop's default: leave the changes behind (stash).
        self.stash_ui.dialog = Some(StashDialog::Switch { branch, stash: true });
        cx.notify();
    }

    pub fn close_stash_dialog(&mut self, cx: &mut Context<Self>) {
        if self.stash_ui.dialog.take().is_some() {
            cx.notify();
        }
    }

    pub fn stash_confirm(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.stash_ui.dialog.take() else { return };
        cx.notify();
        let Some(repo) = self.repo.clone() else { return };
        let text = self.stash_ui.input.read(cx).as_str().trim().to_string();
        match d {
            StashDialog::Create { selected_only } => {
                let paths = if selected_only { stash_paths(repo.read(cx)) } else { Vec::new() };
                repo.update(cx, |r, cx| r.stash_push(text, paths, cx));
            }
            StashDialog::Rename(s) => repo.update(cx, |r, cx| r.stash_rename(s, text, cx)),
            StashDialog::Drop(s) => repo.update(cx, |r, cx| r.stash_drop(s.sha, cx)),
            StashDialog::Switch { branch, stash } => {
                repo.update(cx, |r, cx| r.checkout_with_stash(branch, stash, cx))
            }
        }
    }

    pub fn render_stash_dialog(&mut self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let dialog = self.stash_ui.dialog.as_ref()?;
        let ws = cx.entity();
        let repo = self.repo.clone()?;
        let r = repo.read(cx);
        let current = r.branch_name();
        let input = self.stash_ui.input.clone();
        let message_field = |placeholder: &str, cx: &mut Context<Self>| {
            div()
                .capture_action(cx.listener(|this, _: &Enter, _, cx| {
                    cx.stop_propagation();
                    this.stash_confirm(cx)
                }))
                .child(text_field("stash-message", &input, placeholder, cx))
        };
        let d = match dialog {
            StashDialog::Create { selected_only } => {
                let n = r.changes().len();
                // Stashing works on whole files, partially selected ones included.
                let m = r.changes().iter().filter(|c| !r.excluded.contains(&c.path)).count();
                let partial = m > 0 && m < n;
                let selected_only = *selected_only;
                let count = if selected_only { m } else { n };
                let mut content = div().flex().flex_col().gap(px(10.0)).pt(px(4.0));
                if partial {
                    let ws1 = ws.clone();
                    let ws2 = ws.clone();
                    content = content
                        .child(option_row(
                            "stash-scope-all",
                            !selected_only,
                            format!("All changes ({n} file{})", plural(n as u32)),
                            None,
                            move |_, cx| {
                                ws1.update(cx, |t, cx| {
                                    t.stash_ui.dialog = Some(StashDialog::Create { selected_only: false });
                                    cx.notify()
                                })
                            },
                            cx,
                        ))
                        .child(option_row(
                            "stash-scope-selected",
                            selected_only,
                            format!("Only the checked files ({m})"),
                            None,
                            move |_, cx| {
                                ws2.update(cx, |t, cx| {
                                    t.stash_ui.dialog = Some(StashDialog::Create { selected_only: true });
                                    cx.notify()
                                })
                            },
                            cx,
                        ));
                }
                content = content.child(message_field("Message (optional)", cx));
                Dialog::new("stash-dialog", if selected_only { "Stash selected files" } else { "Stash all changes" })
                    .body(format!(
                        "Save {count} changed file{} to a new stash and restore a clean working tree.",
                        plural(count as u32)
                    ))
                    .detail(format!("Untracked files are included. You can restore it any time from the Stashes list on {current}."))
                    .content(content)
                    .confirm_label("Stash changes")
            }
            StashDialog::Rename(s) => Dialog::new("stash-dialog", "Rename stash")
                .body(format!("A new message for “{}”.", s.title()))
                .detail("The stash moves to the top of the list.")
                .content(div().pt(px(4.0)).child(message_field("Message", cx)))
                .confirm_label("Rename"),
            StashDialog::Drop(s) => Dialog::new("stash-dialog", "Discard this stash?")
                .body(format!("Are you sure you want to discard “{}”?", s.title()))
                .detail("This can't be undone. The stashed changes are deleted.")
                .confirm_label("Discard stash")
                .danger(),
            StashDialog::Switch { branch, stash } => {
                let (stash, target) = (*stash, branch.name.clone());
                let ws1 = ws.clone();
                let ws2 = ws.clone();
                let (b1, b2) = (branch.clone(), branch.clone());
                let content = div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .pt(px(4.0))
                    .child(option_row(
                        "switch-stash",
                        stash,
                        format!("Leave my changes on {current}"),
                        Some("Your in-progress work will be stashed on this branch so you can come back to it later.".into()),
                        move |_, cx| {
                            ws1.update(cx, |t, cx| {
                                t.stash_ui.dialog = Some(StashDialog::Switch { branch: b1.clone(), stash: true });
                                cx.notify()
                            })
                        },
                        cx,
                    ))
                    .child(option_row(
                        "switch-bring",
                        !stash,
                        format!("Bring my changes to {target}"),
                        Some("Your in-progress work will follow you to the new branch.".into()),
                        move |_, cx| {
                            ws2.update(cx, |t, cx| {
                                t.stash_ui.dialog = Some(StashDialog::Switch { branch: b2.clone(), stash: false });
                                cx.notify()
                            })
                        },
                        cx,
                    ));
                Dialog::new("switch-branch-dialog", format!("Switch to {target}"))
                    .body(format!("You have changes on {current}. What would you like to do with them?"))
                    .content(content)
                    .confirm_label("Switch branch")
            }
        };
        let ws2 = ws.clone();
        Some(
            d.on_confirm(move |_, cx| ws.update(cx, |this, cx| this.stash_confirm(cx)))
                .on_cancel(move |_, cx| ws2.update(cx, |this, cx| this.close_stash_dialog(cx))),
        )
    }

    // ---- sidebar panel ----------------------------------------------------------

    /// The collapsible "Stashes (N)" panel. `None` when the repo has no stashes.
    pub fn render_stash_panel(&mut self, repo: &Entity<RepoModel>, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let theme = Theme::of(cx).clone();
        let r = repo.read(cx);
        let st = &r.stashes;
        if st.list.is_empty() {
            return None;
        }
        let branch = r.status.as_ref().and_then(|s| s.branch.clone());
        let visible = st.visible(branch.as_deref());
        let n = visible.len();
        let collapsed = st.collapsed;
        let all = st.all_branches;
        let total = st.list.len();
        let viewing = st.viewing;
        let selected = st.selected.clone();
        let (repo_a, repo_b) = (repo.clone(), repo.clone());

        let header = div()
            .flex()
            .flex_none()
            .items_center()
            .h(px(34.0))
            .pl(px(6.0))
            .pr(px(8.0))
            .gap(px(4.0))
            .child(
                pressable("stash-toggle", false, cx)
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap(px(6.0))
                    .h(px(26.0))
                    .px(px(6.0))
                    .rounded(px(6.0))
                    .child(
                        icon(if collapsed { Icon::ChevronRight } else { Icon::ChevronDown })
                            .size(px(12.0))
                            .text_color(theme.text_faint),
                    )
                    .child(icon(Icon::Archive).size(px(14.0)).text_color(theme.text_muted))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.text_muted)
                            .child(format!("Stashes ({n})")),
                    )
                    .on_click(move |_, _, cx| repo_a.update(cx, |r, cx| r.toggle_stash_panel(cx))),
            )
            .child(
                Button::new("stash-filter")
                    .ghost()
                    .compact()
                    .label(if all { "All branches" } else { "This branch" })
                    .on_click(move |_, _, cx| repo_b.update(cx, |r, cx| r.toggle_stash_filter(cx))),
            );

        let body: Option<gpui::AnyElement> = if collapsed {
            None
        } else if n == 0 {
            let repo_c = repo.clone();
            Some(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(14.0))
                    .pb(px(10.0))
                    .text_size(px(12.0))
                    .text_color(theme.text_faint)
                    .child("No stashes on this branch")
                    .child(
                        Button::new("stash-show-all")
                            .ghost()
                            .compact()
                            .label(format!("Show all ({total})"))
                            .on_click(move |_, _, cx| repo_c.update(cx, |r, cx| r.toggle_stash_filter(cx))),
                    )
                    .into_any_element(),
            )
        } else {
            let list_repo = repo.clone();
            let ws = cx.entity();
            let scroll = r.stashes.list_scroll.clone();
            let visible_for_list = visible.clone();
            let branch_for_list = branch.clone();
            let height = n.min(STASH_MAX_ROWS) as f32 * STASH_ROW + 4.0;
            Some(
                div()
                    .key_context(LIST)
                    .h(px(height))
                    .flex_none()
                    .child(
                        uniform_list("stashes", n, move |range, _w, cx| {
                            let theme = Theme::of(cx).clone();
                            let r = list_repo.read(cx);
                            range
                                .filter_map(|i| {
                                    let s = r.stashes.list.get(*visible_for_list.get(i)?)?.clone();
                                    let is_sel = viewing && selected.as_deref() == Some(s.sha.as_str());
                                    let here = s.branch.is_some() && s.branch == branch_for_list;
                                    let repo = list_repo.clone();
                                    let ws = ws.clone();
                                    let sha = s.sha.clone();
                                    let menu_stash = s.clone();
                                    Some(
                                        pressable(("stash", s.index), is_sel, cx)
                                            .flex()
                                            .flex_col()
                                            .justify_center()
                                            .gap(px(3.0))
                                            .h(px(STASH_ROW))
                                            .w_full()
                                            .px(px(10.0))
                                            .rounded(px(6.0))
                                            .child(
                                                div()
                                                    .truncate()
                                                    .text_size(px(13.0))
                                                    .text_color(if here || is_sel { theme.text } else { theme.text_muted })
                                                    .child(s.title()),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(5.0))
                                                    .text_size(px(11.5))
                                                    .text_color(theme.text_faint)
                                                    .child(
                                                        icon(Icon::GitBranch)
                                                            .size(px(11.0))
                                                            .text_color(if here { theme.accent } else { theme.text_faint }),
                                                    )
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
                                                        d.child(div().flex_1()).child(
                                                            badge("this branch", cx).h(px(16.0)).text_size(px(10.5)).text_color(theme.accent),
                                                        )
                                                    }),
                                            )
                                            .on_mouse_down(gpui::MouseButton::Right, move |e, w, cx| {
                                                let pos = e.position;
                                                let s = menu_stash.clone();
                                                ws.update(cx, |this, cx| this.open_stash_menu(s, pos, w, cx))
                                            })
                                            .on_click(move |_, _, cx| {
                                                let sha = sha.clone();
                                                repo.update(cx, |r, cx| r.select_stash(sha, cx))
                                            }),
                                    )
                                })
                                .collect()
                        })
                        .track_scroll(&scroll)
                        .px(px(6.0))
                        .size_full(),
                    )
                    .into_any_element(),
            )
        };

        Some(
            div()
                .flex()
                .flex_col()
                .flex_none()
                .border_t_1()
                .border_color(theme.border)
                .child(header)
                .children(body)
                .into_any_element(),
        )
    }

    // ---- main pane --------------------------------------------------------------

    pub fn render_stash_main(&mut self, repo: &Entity<RepoModel>, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = Theme::of(cx).clone();
        let r = repo.read(cx);
        let Some(stash) = r.stashes.selected_stash().cloned() else {
            return div().flex_1().into_any_element();
        };
        let here = stash.branch.is_some() && stash.branch == r.status.as_ref().and_then(|s| s.branch.clone());
        let n_files = r.stashes.files.len();
        let diff = r.stashes.diff.clone();
        let hl = r.stashes.hl.clone();
        let file_key = r
            .stashes
            .selected_file
            .and_then(|ix| r.stashes.files.get(ix))
            .map(|f| f.path.clone())
            .unwrap_or_default();
        let files_scroll = r.stashes.files_scroll.clone();
        let list_repo = repo.clone();
        let (r1, r2, r3) = (repo.clone(), repo.clone(), repo.clone());
        let (sha1, sha2, sha3) = (stash.sha.clone(), stash.sha.clone(), stash.clone());
        let ws = cx.entity();

        let header = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(16.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(6.0))
                    .child(
                        div()
                            .truncate()
                            .text_size(px(15.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(stash.title()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .text_size(px(12.0))
                            .text_color(theme.text_faint)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(5.0))
                                    .child(icon(Icon::GitBranch).size(px(12.0)).text_color(if here { theme.accent } else { theme.text_faint }))
                                    .child(stash.branch.clone().unwrap_or_else(|| "(no branch)".into())),
                            )
                            .child(badge(if here { "this branch" } else { "other branch" }, cx).text_color(if here {
                                theme.accent
                            } else {
                                theme.text_muted
                            }))
                            .child(format!("{} • {}", stash.date, relative_time(stash.time)))
                            .child(format!("{n_files} file{}", plural(n_files as u32)))
                            .child(div().font_family(theme.font_mono.clone()).child(stash.short_sha.clone())),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .gap(px(8.0))
                    .child(
                        Button::new("stash-apply")
                            .icon(Icon::ArchiveUp)
                            .label("Apply")
                            .on_click(move |_, _, cx| {
                                let sha = sha1.clone();
                                r1.update(cx, |r, cx| r.stash_apply(sha, cx))
                            }),
                    )
                    .child(
                        Button::new("stash-pop")
                            .primary()
                            .icon(Icon::Restart)
                            .label("Restore")
                            .on_click(move |_, _, cx| {
                                let sha = sha2.clone();
                                r2.update(cx, |r, cx| r.stash_pop(sha, cx))
                            }),
                    )
                    .child(
                        Button::new("stash-drop")
                            .ghost()
                            .icon(Icon::Trash)
                            .label("Discard")
                            .on_click(move |_, w, cx| {
                                let s = sha3.clone();
                                let _ = &r3;
                                ws.update(cx, |this, cx| this.open_stash_dialog(StashDialog::Drop(s), w, cx))
                            }),
                    ),
            );

        let files = uniform_list("stash-files", n_files, move |range, _w, cx| {
            let theme = Theme::of(cx).clone();
            let r = list_repo.read(cx);
            range
                .filter_map(|ix| {
                    let f = r.stashes.files.get(ix)?;
                    let selected = r.stashes.selected_file == Some(ix);
                    let (dir, name) = split_path(&f.path);
                    let repo = list_repo.clone();
                    Some(
                        pressable(("stash-file", ix), selected, cx)
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .h(px(28.0))
                            .w_full()
                            .px(px(8.0))
                            .rounded(px(6.0))
                            .text_size(px(12.5))
                            .child(
                                div()
                                    .flex()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(div().flex_shrink(1.0).min_w_0().truncate().text_color(theme.text_faint).child(dir.to_string()))
                                    .child(div().flex_none().text_color(if selected { theme.text } else { theme.text_muted }).child(name.to_string())),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .w(px(14.0))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(status_color(f.status, cx))
                                    .child(f.status.badge()),
                            )
                            .on_click(move |_, _, cx| repo.update(cx, |r, cx| r.select_stash_file(ix, cx))),
                    )
                })
                .collect()
        })
        .track_scroll(&files_scroll)
        .px(px(6.0))
        .flex_1();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .child(header)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(div().key_context(LIST).flex().flex_col().flex_none().w(px(260.0)).py(px(4.0)).child(files))
                    .child(vdivider(cx))
                    .child(self.render_diff(
                        diff,
                        hl,
                        SharedString::from(format!("stash-diff-{}-{file_key}", stash.sha)),
                        None,
                        cx,
                    )),
            )
            .into_any_element()
    }
}

/// Pathspecs for "stash the checked files": include the old path of renames
/// so both halves of the rename are stashed.
fn stash_paths(r: &RepoModel) -> Vec<String> {
    let mut v = Vec::new();
    for c in r.changes() {
        if r.excluded.contains(&c.path) {
            continue;
        }
        v.push(c.path.clone());
        if let Some(old) = &c.old_path {
            v.push(old.clone());
        }
    }
    v
}

/// A radio-style option card used by the stash and switch-branch dialogs.
fn option_row(
    id: impl Into<ElementId>,
    selected: bool,
    title: String,
    detail: Option<String>,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> gpui::AnyElement {
    let theme = Theme::of(cx).clone();
    pressable(id, selected, cx)
        .flex()
        .items_start()
        .gap(px(10.0))
        .px(px(12.0))
        .py(px(9.0))
        .rounded(px(8.0))
        .border_1()
        .border_color(if selected { theme.border_strong } else { theme.border })
        .child(
            div()
                .flex_none()
                .mt(px(2.0))
                .size(px(14.0))
                .rounded_full()
                .border_1()
                .border_color(if selected { theme.solid } else { theme.border_strong })
                .flex()
                .items_center()
                .justify_center()
                .when(selected, |d| d.child(div().size(px(8.0)).rounded_full().bg(theme.solid))),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .min_w_0()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(title),
                )
                .children(detail.map(|d| div().text_size(px(12.0)).text_color(theme.text_faint).child(d))),
        )
        .on_click(move |_, w, cx| on_click(w, cx))
        .into_any_element()
}
