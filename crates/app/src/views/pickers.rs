//! Toolbar popovers: repository list, branch list, new branch, clone.
//! Filter input on top, virtualized list, footer actions. ↑/↓ + Enter work.

use crate::shared::relative_time::relative_time;
use gd_git::BranchKind;
use gd_ui::{Button, Icon, Theme, divider, icon, icon_button, pressable};
use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, Window, div,
    prelude::*, px, uniform_list,
};
use gpui_elements::editable_text::actions::{Enter, NavDown, NavUp};

use crate::actions::{SelectNext, SelectPrevious};
use crate::workspace::{Popover, Workspace, text_field};

const PICKER_ROW: f32 = 34.0;
const PICKER_MAX_ROWS: usize = 10;

impl Workspace {
    fn filter_text(&self, cx: &gpui::App) -> String {
        self.filter.read(cx).as_str().trim().to_lowercase()
    }

    /// Indices into `settings.repositories` matching the filter.
    fn filtered_repos(&self, cx: &gpui::App) -> Vec<usize> {
        let f = self.filter_text(cx);
        self.settings
            .repositories
            .iter()
            .enumerate()
            .filter(|(_, p)| f.is_empty() || p.display().to_string().to_lowercase().contains(&f))
            .map(|(i, _)| i)
            .collect()
    }

    /// Indices into `repo.branches` matching the filter.
    fn filtered_branches(&self, cx: &gpui::App) -> Vec<usize> {
        let f = self.filter_text(cx);
        let Some(repo) = &self.repo else { return Vec::new() };
        let r = repo.read(cx);
        let local: std::collections::HashSet<&str> = r
            .branches
            .iter()
            .filter(|b| b.kind == BranchKind::Local)
            .map(|b| b.name.as_str())
            .collect();
        r.branches
            .iter()
            .enumerate()
            .filter(|(_, b)| f.is_empty() || b.name.to_lowercase().contains(&f))
            // Hide remote branches that already have a local counterpart.
            .filter(|(_, b)| {
                b.kind == BranchKind::Local
                    || !b
                        .name
                        .split_once('/')
                        .is_some_and(|(_, n)| local.contains(n))
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn picker_shell(
        &mut self,
        title: &'static str,
        placeholder: &'static str,
        n: usize,
        list: impl IntoElement,
        footer: impl IntoElement,
        on_enter: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = Theme::of(cx).clone();
        let height = (n.clamp(1, PICKER_MAX_ROWS) as f32) * PICKER_ROW + 8.0;
        div()
            .flex()
            .flex_col()
            .capture_action(cx.listener(move |this, _: &Enter, w, cx| {
                cx.stop_propagation();
                on_enter(this, w, cx)
            }))
            // The focused text input turns ↑/↓ into NavUp/NavDown, so catch those
            // too (the LIST-context bindings never reach a focused input).
            .capture_action(cx.listener(|this, _: &NavDown, _, cx| {
                this.popover_selected += 1;
                this.popover_scroll
                    .scroll_to_item(this.popover_selected, gpui::ScrollStrategy::Nearest);
                cx.stop_propagation();
                cx.notify()
            }))
            .capture_action(cx.listener(|this, _: &NavUp, _, cx| {
                this.popover_selected = this.popover_selected.saturating_sub(1);
                this.popover_scroll
                    .scroll_to_item(this.popover_selected, gpui::ScrollStrategy::Nearest);
                cx.stop_propagation();
                cx.notify()
            }))
            .capture_action(cx.listener(|this, _: &SelectNext, _, cx| {
                this.popover_selected += 1;
                this.popover_scroll
                    .scroll_to_item(this.popover_selected, gpui::ScrollStrategy::Nearest);
                cx.stop_propagation();
                cx.notify()
            }))
            .capture_action(cx.listener(|this, _: &SelectPrevious, _, cx| {
                this.popover_selected = this.popover_selected.saturating_sub(1);
                this.popover_scroll
                    .scroll_to_item(this.popover_selected, gpui::ScrollStrategy::Nearest);
                cx.stop_propagation();
                cx.notify()
            }))
            .child(
                div()
                    .px(px(14.0))
                    .pt(px(12.0))
                    .pb(px(8.0))
                    .text_size(px(12.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text_muted)
                    .child(title),
            )
            .child(
                div()
                    .px(px(10.0))
                    .pb(px(10.0))
                    .child(text_field("picker-filter", &self.filter, placeholder, cx)),
            )
            .child(divider(cx))
            .child(
                div()
                    .h(px(height))
                    .py(px(4.0))
                    .when(n == 0, |d| {
                        d.flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(12.0))
                            .text_color(theme.text_faint)
                            .child("No matches")
                    })
                    .when(n > 0, |d| d.child(list)),
            )
            .child(divider(cx))
            .child(
                div()
                    .flex()
                    .gap(px(6.0))
                    .p(px(8.0))
                    .bg(gd_ui::theme::ink(0.02))
                    .child(footer),
            )
            .into_any_element()
    }

    pub fn render_repo_list(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let items = self.filtered_repos(cx);
        let n = items.len();
        self.popover_selected = self.popover_selected.min(n.saturating_sub(1));
        let selected = self.popover_selected;
        let current = self.settings.selected;
        let this = cx.entity();
        let rows = items.clone();
        let list = uniform_list("repo-list", n, move |range, _w, cx| {
            let theme = Theme::of(cx).clone();
            let ws = this.read(cx);
            range
                .filter_map(|i| {
                    let ix = *rows.get(i)?;
                    let path = ws.settings.repositories.get(ix)?;
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let parent = path.parent().map(|p| p.display().to_string()).unwrap_or_default();
                    let open = this.clone();
                    let remove = this.clone();
                    Some(
                        pressable(("repo", ix), i == selected, cx)
                            .group("repo-row")
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .h(px(PICKER_ROW))
                            .w_full()
                            .px(px(8.0))
                            .rounded(px(6.0))
                            .child(icon(if current == Some(ix) { Icon::Check } else { Icon::Folder })
                                .size(px(14.0))
                                .text_color(if current == Some(ix) { theme.text } else { theme.text_faint }))
                            .child(div().flex_none().text_color(theme.text).child(name))
                            .child(div().flex_1().min_w_0().truncate().text_size(px(11.5)).text_color(theme.text_faint).child(parent))
                            .child(
                                div()
                                    .invisible()
                                    .group_hover_style("repo-row", |s| s.visible())
                                    .child(icon_button(("remove-repo", ix), Icon::Close).compact().on_click(
                                        move |_, _, cx| {
                                            cx.stop_propagation();
                                            remove.update(cx, |ws, cx| ws.remove_repository(ix, cx))
                                        },
                                    )),
                            )
                            .on_click(move |_, _, cx| open.update(cx, |ws, cx| ws.open_repository(ix, cx))),
                    )
                })
                .collect()
        })
        .track_scroll(&self.popover_scroll)
            .px(px(6.0))
        .size_full();

        let footer = div()
            .flex()
            .gap(px(6.0))
            .w_full()
            .child(
                Button::new("add-repo")
                    .icon(Icon::Plus)
                    .label("Add existing…")
                    .compact()
                    .on_click(cx.listener(|this, _, _, cx| this.prompt_add_repository(cx))),
            )
            .child(
                Button::new("clone-repo")
                    .icon(Icon::Cloud)
                    .label("Clone…")
                    .compact()
                    .on_click(cx.listener(|this, _, w, cx| this.toggle_popover(Popover::Clone, w, cx))),
            );

        self.picker_shell(
            "Repositories",
            "Filter repositories",
            n,
            list,
            footer,
            move |this, _w, cx| {
                if let Some(&ix) = this.filtered_repos(cx).get(this.popover_selected) {
                    this.open_repository(ix, cx)
                }
            },
            cx,
        )
    }

    pub fn render_branch_list(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let items = self.filtered_branches(cx);
        let n = items.len();
        self.popover_selected = self.popover_selected.min(n.saturating_sub(1));
        let selected = self.popover_selected;
        let Some(repo) = self.repo.clone() else {
            return div().into_any_element();
        };
        let rows = items.clone();
        let list_repo = repo.clone();
        let ws = cx.entity();
        let list = uniform_list("branch-list", n, move |range, _w, cx| {
            let theme = Theme::of(cx).clone();
            let r = list_repo.read(cx);
            range
                .filter_map(|i| {
                    let b = r.branches.get(*rows.get(i)?)?.clone();
                    let ws = ws.clone();
                    let repo = list_repo.clone();
                    let (ws_menu, b_menu) = (ws.clone(), b.clone());
                    Some(
                        pressable(("branch", i), i == selected, cx)
                            .on_mouse_down(gpui::MouseButton::Right, move |e, w, cx| {
                                let (pos, b) = (e.position, b_menu.clone());
                                ws_menu.update(cx, |this, cx| this.open_branch_menu(b, pos, w, cx))
                            })
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .h(px(PICKER_ROW))
                            .w_full()
                            .px(px(8.0))
                            .rounded(px(6.0))
                            .child(
                                icon(if b.is_head { Icon::Check } else if b.kind == BranchKind::Remote { Icon::Cloud } else { Icon::GitBranch })
                                    .size(px(14.0))
                                    .text_color(if b.is_head { theme.text } else { theme.text_faint }),
                            )
                            .child(div().flex_1().min_w_0().truncate().text_color(if b.is_head { theme.text } else { theme.text_muted }).child(b.name.clone()))
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(px(11.0))
                                    .text_color(theme.text_faint)
                                    .child(relative_time(b.tip_time)),
                            )
                            .on_click(move |_, w, cx| {
                                if !b.is_head {
                                    repo.update(cx, |r, cx| r.checkout(b.clone(), cx));
                                }
                                ws.update(cx, |ws, cx| ws.close_popover(w, cx));
                            }),
                    )
                })
                .collect()
        })
        .track_scroll(&self.popover_scroll)
            .px(px(6.0))
        .size_full();

        let footer = div()
            .flex()
            .gap(px(6.0))
            .w_full()
            .child(
                Button::new("new-branch")
                    .icon(Icon::Plus)
                    .label("New branch")
                    .compact()
                    .on_click(cx.listener(|this, _, w, cx| {
                        let prefill = this.filter.read(cx).as_str().to_string();
                        this.toggle_popover(Popover::NewBranch, w, cx);
                        this.name_input.update(cx, |s, cx| s.emplace(&prefill, cx));
                    })),
            )
            .child(
                Button::new("merge-into-current")
                    .icon(Icon::Fork)
                    .label("Merge into current branch…")
                    .compact()
                    .on_click(cx.listener(|this, _, w, cx| {
                        this.open_merge_dialog(crate::merge_ops::MergeMode::Merge, None, w, cx)
                    })),
            );

        self.picker_shell(
            "Branches",
            "Filter branches",
            n,
            list,
            footer,
            move |this, w, cx| {
                let Some(repo) = this.repo.clone() else { return };
                let Some(&ix) = this.filtered_branches(cx).get(this.popover_selected) else { return };
                let b = repo.read(cx).branches[ix].clone();
                if !b.is_head {
                    repo.update(cx, |r, cx| r.checkout(b, cx));
                }
                this.close_popover(w, cx);
            },
            cx,
        )
        .into_any_element()
    }

    pub fn render_new_branch(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = Theme::of(cx).clone();
        let name = sanitize_branch_name(self.name_input.read(cx).as_str());
        let base = self
            .repo
            .as_ref()
            .map(|r| r.read(cx).branch_name())
            .unwrap_or_default();
        let submit = |this: &mut Self, w: &mut Window, cx: &mut Context<Self>| {
            let name = sanitize_branch_name(this.name_input.read(cx).as_str());
            if name.is_empty() {
                return;
            }
            if let Some(repo) = &this.repo {
                repo.update(cx, |r, cx| r.create_branch(name, cx));
            }
            this.close_popover(w, cx);
        };
        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .p(px(14.0))
            .capture_action(cx.listener(move |this, _: &Enter, w, cx| {
                cx.stop_propagation();
                submit(this, w, cx)
            }))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text_muted)
                    .child("Create a branch"),
            )
            .child(text_field("branch-name", &self.name_input, "Branch name", cx))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme.text_faint)
                    .child(if name.is_empty() {
                        SharedString::from(format!("Based on {base}"))
                    } else {
                        format!("Creates {name} from {base} and switches to it").into()
                    }),
            )
            .child(
                Button::new("create-branch")
                    .primary()
                    .full_width()
                    .label("Create branch")
                    .disabled(name.is_empty())
                    .on_click(cx.listener(move |this, _, w, cx| submit(this, w, cx))),
            )
    }

    pub fn render_clone(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = Theme::of(cx).clone();
        let url = self.url_input.read(cx).as_str().trim().to_string();
        let submit = |this: &mut Self, _w: &mut Window, cx: &mut Context<Self>| {
            let url = this.url_input.read(cx).as_str().to_string();
            this.clone_repository(url, cx)
        };
        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .p(px(14.0))
            .capture_action(cx.listener(move |this, _: &Enter, w, cx| {
                cx.stop_propagation();
                submit(this, w, cx)
            }))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text_muted)
                    .child("Clone a repository"),
            )
            .child(text_field("clone-url", &self.url_input, "https://github.com/owner/repo.git", cx))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme.text_faint)
                    .child("You'll pick the parent folder next."),
            )
            .child(
                Button::new("do-clone")
                    .primary()
                    .full_width()
                    .label("Clone")
                    .disabled(url.is_empty())
                    .on_click(cx.listener(move |this, _, w, cx| submit(this, w, cx))),
            )
    }
}

/// Desktop-style branch name sanitizing: spaces → `-`, strip invalid chars.
pub fn sanitize_branch_name(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.trim().chars() {
        match ch {
            c if c.is_whitespace() => out.push('-'),
            '~' | '^' | ':' | '?' | '*' | '[' | '\\' => {}
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    while out.contains("..") {
        out = out.replace("..", ".");
    }
    out.trim_matches(['/', '.', '-']).to_string()
}
